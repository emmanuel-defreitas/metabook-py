"""
Tests for the tokenizer resolution service and token counting.

Everything here runs offline: the counter is exercised with a stubbed encoder
callable, and the tokenizer module with a tiny tokenizer file built in memory
(the Hub download function is monkeypatched). The single real cold-start
fetch lives in the `integration`-marked test at the bottom and is skipped
unless METABOOK_TOKENIZER_IT is set.
"""

import os

import httpx
import pytest

from metabook_py.core.exceptions import TokenizerNotFoundError, TokenizerUnavailableError
from metabook_py.services import tokenizers as tokenizers_service
from metabook_py.services.counter import build_structure_tree
from metabook_py.services.detector import detect_schema
from metabook_py.services.tokenizers import TokenEncoder, _load_encoder, get_encoder


@pytest.fixture(autouse=True)
def clear_encoder_cache():
    _load_encoder.cache_clear()
    yield
    _load_encoder.cache_clear()


def stub_encoder(text: str) -> int:
    """Deterministic stand-in for a real tokenizer: whitespace tokens."""
    return len(text.split())


# ── Counter: token counts with a stubbed encoder ───────────────────────────────


def test_no_encoder_means_no_token_counts(standard_book_raw):
    schema = detect_schema(standard_book_raw)
    nodes, summary = build_structure_tree(standard_book_raw, schema)

    assert summary.total_tokens is None
    for chapter in nodes:
        assert chapter.total_tokens is None
        for para in chapter.paragraphs:
            assert para.token_count is None


def test_token_counts_at_every_level(standard_book_raw):
    schema = detect_schema(standard_book_raw)
    nodes, summary = build_structure_tree(standard_book_raw, schema, encoder=stub_encoder)

    assert summary.total_tokens is not None and summary.total_tokens > 0
    for chapter in nodes:
        assert chapter.total_tokens is not None
        for para in chapter.paragraphs:
            assert para.token_count is not None and para.token_count > 0


def test_parent_totals_equal_sum_of_children(standard_book_raw):
    schema = detect_schema(standard_book_raw)
    nodes, summary = build_structure_tree(standard_book_raw, schema, encoder=stub_encoder)

    for chapter in nodes:
        assert chapter.total_tokens == sum(p.token_count for p in chapter.paragraphs)
    assert summary.total_tokens == sum(c.total_tokens for c in nodes)


def test_deep_detail_carries_token_counts(standard_book_raw):
    schema = detect_schema(standard_book_raw)
    nodes, _ = build_structure_tree(
        standard_book_raw, schema, detail="clause", encoder=stub_encoder
    )

    sentences = [s for c in nodes for p in c.paragraphs for s in p.sentences]
    assert sentences
    for sentence in sentences:
        assert sentence.token_count is not None and sentence.token_count > 0
        for clause in sentence.clauses:
            assert clause.token_count is not None


def test_scripture_tree_is_additive(scripture_raw):
    schema = detect_schema(scripture_raw)
    parts, summary = build_structure_tree(scripture_raw, schema, encoder=stub_encoder)

    for part in parts:
        assert part.total_tokens == sum(c.total_tokens for c in part.children)
        for chapter in part.children:
            assert chapter.total_tokens == sum(v.token_count for v in chapter.paragraphs)
    assert summary.total_tokens == sum(p.total_tokens for p in parts)


def test_token_fields_omitted_from_serialization_without_encoder(flat_raw):
    schema = detect_schema(flat_raw)
    nodes, summary = build_structure_tree(flat_raw, schema)

    dumped = summary.model_dump()
    assert "total_tokens" not in dumped
    assert "token_count" not in nodes[0].model_dump()

    nodes, summary = build_structure_tree(flat_raw, schema, encoder=stub_encoder)
    assert "total_tokens" in summary.model_dump()
    assert "token_count" in nodes[0].model_dump()


# ── Tokenizer module: resolution with a stubbed Hub ────────────────────────────


@pytest.fixture
def tiny_tokenizer_file(tmp_path) -> str:
    """A real (WordLevel) tokenizer file written locally — no network. Its
    post-processor adds [CLS]/[SEP] so we can prove special tokens are
    excluded from counts."""
    from tokenizers import Tokenizer
    from tokenizers.models import WordLevel
    from tokenizers.pre_tokenizers import Whitespace
    from tokenizers.processors import TemplateProcessing

    vocab = {"[UNK]": 0, "[CLS]": 1, "[SEP]": 2, "hello": 3, "world": 4}
    tokenizer = Tokenizer(WordLevel(vocab, unk_token="[UNK]"))
    tokenizer.pre_tokenizer = Whitespace()
    tokenizer.post_processor = TemplateProcessing(
        single="[CLS] $A [SEP]",
        special_tokens=[("[CLS]", 1), ("[SEP]", 2)],
    )
    path = tmp_path / "tokenizer.json"
    tokenizer.save(str(path))
    return str(path)


@pytest.fixture
def stub_hub(monkeypatch, tiny_tokenizer_file):
    """Route _download_tokenizer_file to the local file, counting calls."""
    calls: list[str] = []

    def fake_download(name: str) -> str:
        calls.append(name)
        return tiny_tokenizer_file

    monkeypatch.setattr(tokenizers_service, "_download_tokenizer_file", fake_download)
    return calls


def test_get_encoder_returns_counting_callable(stub_hub):
    encoder = get_encoder("stub/wordlevel")
    assert isinstance(encoder, TokenEncoder)
    assert encoder.name == "stub/wordlevel"
    assert encoder.vocab_size == 5


def test_special_tokens_are_excluded(stub_hub):
    encoder = get_encoder("stub/wordlevel")
    # The post-processor would add [CLS] and [SEP] (count 4); we expect 2.
    assert encoder("hello world") == 2
    assert encoder("hello unknown world") == 3  # [UNK] still counts as content


def test_resolution_is_memoized_per_name(stub_hub):
    first = get_encoder("stub/wordlevel")
    second = get_encoder("stub/wordlevel")
    assert first is second
    assert stub_hub == ["stub/wordlevel"]

    get_encoder("stub/other")
    assert stub_hub == ["stub/wordlevel", "stub/other"]


def test_failed_resolution_is_not_cached(monkeypatch, tiny_tokenizer_file):
    attempts: list[str] = []

    def flaky_download(name: str) -> str:
        attempts.append(name)
        if len(attempts) == 1:
            raise TokenizerUnavailableError(name, "connection reset")
        return tiny_tokenizer_file

    monkeypatch.setattr(tokenizers_service, "_download_tokenizer_file", flaky_download)

    with pytest.raises(TokenizerUnavailableError):
        get_encoder("stub/flaky")
    assert get_encoder("stub/flaky").vocab_size == 5  # retry succeeded
    assert len(attempts) == 2


# ── Tokenizer module: Hub error taxonomy → our exceptions ──────────────────────


def _patch_hub_download(monkeypatch, exc: Exception):
    def raiser(*args, **kwargs):
        raise exc

    monkeypatch.setattr("huggingface_hub.hf_hub_download", raiser)


def _http_404() -> httpx.Response:
    return httpx.Response(404, request=httpx.Request("GET", "https://huggingface.co"))


def test_unknown_repository_is_not_found(monkeypatch):
    from huggingface_hub.errors import RepositoryNotFoundError

    _patch_hub_download(
        monkeypatch, RepositoryNotFoundError("404 Client Error", response=_http_404())
    )
    with pytest.raises(TokenizerNotFoundError) as excinfo:
        get_encoder("no-such/tokenizer")
    assert excinfo.value.name == "no-such/tokenizer"


def test_repo_without_tokenizer_file_is_not_found(monkeypatch):
    from huggingface_hub.errors import EntryNotFoundError

    _patch_hub_download(monkeypatch, EntryNotFoundError("404 Client Error"))
    with pytest.raises(TokenizerNotFoundError):
        get_encoder("some/dataset-repo")


def test_invalid_name_is_not_found(monkeypatch):
    from huggingface_hub.errors import HFValidationError

    _patch_hub_download(monkeypatch, HFValidationError("Repo id must be in the form ..."))
    with pytest.raises(TokenizerNotFoundError):
        get_encoder("not a repo id!!")


def test_offline_cold_start_is_unavailable(monkeypatch):
    from huggingface_hub.errors import LocalEntryNotFoundError

    _patch_hub_download(
        monkeypatch,
        LocalEntryNotFoundError("Cannot reach the Hub and file is not cached"),
    )
    with pytest.raises(TokenizerUnavailableError) as excinfo:
        get_encoder("bert-base-uncased")
    assert excinfo.value.name == "bert-base-uncased"


def test_transport_failure_is_unavailable(monkeypatch):
    _patch_hub_download(monkeypatch, ConnectionError("connection reset by peer"))
    with pytest.raises(TokenizerUnavailableError):
        get_encoder("bert-base-uncased")


# ── Integration: one real cold-start fetch (opt-in) ────────────────────────────


@pytest.mark.integration
@pytest.mark.skipif(
    not os.environ.get("METABOOK_TOKENIZER_IT"),
    reason="real Hugging Face Hub fetch; set METABOOK_TOKENIZER_IT=1 to run",
)
def test_real_cold_start_fetch():
    encoder = get_encoder("bert-base-uncased")
    assert encoder.vocab_size > 30_000
    count = encoder("It is a truth universally acknowledged.")
    assert count > 0
    # Additivity holds for a wordpiece tokenizer: no special tokens counted.
    assert encoder("hello") + encoder("world") == encoder("hello world")

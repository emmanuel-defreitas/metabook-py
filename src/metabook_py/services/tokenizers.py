"""
Tokenizer resolution service.

Responsibilities
----------------
- Resolve a Hugging Face tokenizer repository name (e.g. "bert-base-uncased")
  into a :class:`TokenEncoder` — a callable that maps text → token count with
  special tokens excluded.
- Resolution is lazy and memoized per process: the first request for a given
  name downloads the tokenizer vocabulary from the Hugging Face Hub (cached on
  disk by huggingface_hub), and subsequent requests reuse the in-memory
  instance. Failed resolutions are NOT cached, so a transient network error on
  cold start can be retried.

The counter service never imports this module; it accepts an encoder callable
as an argument, so it stays dependency-free and offline-testable. All heavy
imports (tokenizers, huggingface_hub) happen inside functions for the same
reason.
"""

from __future__ import annotations

from collections.abc import Callable
from dataclasses import dataclass, field
from functools import cache

from metabook_py.core.exceptions import TokenizerNotFoundError, TokenizerUnavailableError

TOKENIZER_FILENAME = "tokenizer.json"


@dataclass(frozen=True)
class TokenEncoder:
    """A resolved tokenizer. Calling it returns the token count of *text*,
    with special tokens ([CLS], [SEP], BOS/EOS, …) excluded so counts are
    additive up the structure tree."""

    name: str
    vocab_size: int
    encode: Callable[[str], int] = field(repr=False)

    def __call__(self, text: str) -> int:
        return self.encode(text)


def get_encoder(name: str) -> TokenEncoder:
    """
    Return the :class:`TokenEncoder` for the Hugging Face tokenizer *name*.

    Raises
    ------
    TokenizerNotFoundError
        *name* is not a resolvable tokenizer (bad repo id, unknown or gated
        repository, or the repository has no tokenizer file).
    TokenizerUnavailableError
        The tokenizer could not be fetched right now (network / Hub failure)
        and is not present in the local disk cache.
    """
    return _load_encoder(name)


@cache
def _load_encoder(name: str) -> TokenEncoder:
    path = _download_tokenizer_file(name)

    from tokenizers import Tokenizer

    try:
        tokenizer = Tokenizer.from_file(path)
    except Exception as exc:
        raise TokenizerNotFoundError(name, f"unparseable tokenizer file: {exc}") from exc

    def _count(text: str) -> int:
        return len(tokenizer.encode(text, add_special_tokens=False).ids)

    return TokenEncoder(
        name=name,
        vocab_size=tokenizer.get_vocab_size(with_added_tokens=True),
        encode=_count,
    )


def _download_tokenizer_file(name: str) -> str:
    """Fetch (or reuse from the on-disk HF cache) the tokenizer file for *name*,
    mapping huggingface_hub's error taxonomy onto ours: bad-request-shaped
    problems → TokenizerNotFoundError, transient ones → TokenizerUnavailableError."""
    from huggingface_hub import hf_hub_download
    from huggingface_hub.errors import (
        EntryNotFoundError,
        GatedRepoError,
        HfHubHTTPError,
        HFValidationError,
        LocalEntryNotFoundError,
        RepositoryNotFoundError,
    )

    try:
        return hf_hub_download(repo_id=name, filename=TOKENIZER_FILENAME)
    except (RepositoryNotFoundError, GatedRepoError) as exc:
        raise TokenizerNotFoundError(name, "repository not found or not accessible") from exc
    # LocalEntryNotFoundError subclasses EntryNotFoundError, so it must be
    # caught first: it means the Hub was unreachable AND nothing was cached.
    except LocalEntryNotFoundError as exc:
        raise TokenizerUnavailableError(name, str(exc)) from exc
    except EntryNotFoundError as exc:
        raise TokenizerNotFoundError(
            name, f"repository has no '{TOKENIZER_FILENAME}' file"
        ) from exc
    except HFValidationError as exc:
        raise TokenizerNotFoundError(name, f"invalid tokenizer name: {exc}") from exc
    except HfHubHTTPError as exc:
        raise TokenizerUnavailableError(name, str(exc)) from exc
    except Exception as exc:
        # Anything else at this stage is a transport-level failure (DNS,
        # connection reset, TLS, …) — transient, not a bad request.
        raise TokenizerUnavailableError(name, str(exc)) from exc

import pytest

from tf_ref import ParseError, Reference, format_ref, parse


@pytest.mark.parametrize(
    ("value", "escaped"),
    [
        ("Genesis", "Genesis"),
        ("Part One", '"Part One"'),
        ("Intro: Notes", '"Intro: Notes"'),
        ('say "hi"', '"say ""hi"""'),
        ("a/b!c@d", '"a/b!c@d"'),
    ],
)
def test_escaped_section_values_round_trip(value, escaped):
    ref = Reference(None, None, (value,))
    assert format_ref(ref) == escaped
    assert parse(escaped) == ref


def test_short_reference_parses_identity_depth_and_selector():
    assert parse("demo@2026/Vol1:3:4!word2-4", depth=3) == Reference(
        "demo", "2026", ("Vol1", "3", "4"), "word", 2, 4
    )


@pytest.mark.parametrize(
    "text",
    [
        "demo@v1/",
        "!word1",
        "A:B!word0",
        "A:B!word-1",
        "A:B!wordx",
        "A:B!word5-3",
        "A:B!word3-clause1",
        "A:B!",
        "urn:tf:demo@v1:A:B!",
        'A:"unterminated',
    ],
)
def test_malformed_input_quotes_the_offending_fragment(text):
    with pytest.raises(ParseError) as exc:
        parse(text)
    assert repr(text) in str(exc.value)


def test_depth_error_names_extra_section():
    with pytest.raises(ParseError, match="D"):
        parse("A:B:C:D", depth=3)

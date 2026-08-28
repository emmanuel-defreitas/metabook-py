"""
Shared fixtures for all test modules.

Sample texts are intentionally minimal but structurally representative.
All samples include the *** START/END *** boilerplate so stripping is tested too.
"""

import pytest

STANDARD_BOOK = """
*** START OF THE PROJECT GUTENBERG EBOOK PRIDE AND PREJUDICE ***

CHAPTER I

It is a truth universally acknowledged, that a single man in possession
of a good fortune, must be in want of a wife. However little known the
feelings or views of such a man may be on his first entering a neighbourhood,
this truth is so well fixed in the minds of the surrounding families.

"My dear Mr. Bennet," said his lady to him one day, "have you heard that
Netherfield Park is let at last?" Mr. Bennet replied that he had not.

This is a third paragraph in chapter one. It has two sentences. Done.

CHAPTER II

Mr. Bennet was among the earliest of her visitors, and such was the
happiness of such a man. He met his neighbour's joy with great pleasure.

The second paragraph of the second chapter. Another sentence follows here.
And a third sentence wraps things up nicely.

CHAPTER III

A short final chapter. Just one paragraph here. That's it.

*** END OF THE PROJECT GUTENBERG EBOOK PRIDE AND PREJUDICE ***
"""

SECTIONED_BOOK = """
*** START OF THE PROJECT GUTENBERG EBOOK ***

PART I

CHAPTER I

First chapter of part one. It contains sentences. Here is another sentence.

Another paragraph in part one, chapter one. More words follow here.

CHAPTER II

Second chapter of part one. Sentence one. Sentence two.

Paragraph two of chapter two. Continues here with more text.

PART II

CHAPTER I

Opening of part two. This is a sentence. Another follows.

Second paragraph in part two chapter one. Final words here.

*** END OF THE PROJECT GUTENBERG EBOOK ***
"""

ESSAY_COLLECTION = """
*** START OF THE PROJECT GUTENBERG EBOOK ***

THE FIRST ESSAY

Opening paragraph of the first essay. It makes an argument. The argument continues.

Second paragraph follows. More ideas here. A final sentence closes it.

THE SECOND ESSAY

A different topic begins here. New ideas emerge. The paragraph ends.

Continuing the second essay. More text fills the page.

*** END OF THE PROJECT GUTENBERG EBOOK ***
"""

FLAT_TEXT = """
*** START OF THE PROJECT GUTENBERG EBOOK ***

This is the first paragraph of the book. It contains some sentences.
Here is another sentence in the first paragraph. And a third.

This is the second paragraph. It also has sentences in it.
Another sentence follows here. And a third one to complete it.

This is the third paragraph. Short and simple. Just three sentences here.

*** END OF THE PROJECT GUTENBERG EBOOK ***
"""

SCRIPTURE_TEXT = """
*** START OF THE PROJECT GUTENBERG EBOOK ***

GENESIS

CHAPTER I

1:1 In the beginning God created the heaven and the earth.
1:2 And the earth was without form, and void; and darkness was upon the face of the deep.
1:3 And God said, Let there be light: and there was light.
1:4 And God saw the light, that it was good.
1:5 And God called the light Day, and the darkness he called Night.

CHAPTER II

2:1 Thus the heavens and the earth were finished, and all the host of them.
2:2 And on the seventh day God ended his work which he had made.
2:3 And God blessed the seventh day, and sanctified it.

EXODUS

CHAPTER I

1:1 Now these are the names of the children of Israel, which came into Egypt.
1:2 Reuben, Simeon, Levi, and Judah.
1:3 Issachar, Zebulun, and Benjamin.
1:4 Dan, and Naphtali, Gad, and Asher.
1:5 And all the souls that came out of the loins of Jacob were seventy souls.

*** END OF THE PROJECT GUTENBERG EBOOK ***
"""


# ── Fixtures ───────────────────────────────────────────────────────────────────


@pytest.fixture
def standard_book_raw() -> str:
    return STANDARD_BOOK


@pytest.fixture
def sectioned_book_raw() -> str:
    return SECTIONED_BOOK


@pytest.fixture
def essay_collection_raw() -> str:
    return ESSAY_COLLECTION


@pytest.fixture
def flat_raw() -> str:
    return FLAT_TEXT


@pytest.fixture
def scripture_raw() -> str:
    return SCRIPTURE_TEXT

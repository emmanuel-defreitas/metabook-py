class BookNotFoundError(Exception):
    """Raised when Gutendex returns zero results for a query."""

    def __init__(self, query: dict):
        self.query = query
        super().__init__(f"No books found for query: {query}")


class AmbiguousBookError(Exception):
    """Raised when Gutendex returns multiple results and no gutenberg_id was given."""

    def __init__(self, matches: list):
        self.matches = matches
        super().__init__(f"Ambiguous query: {len(matches)} books matched")


class TextUnavailableError(Exception):
    """Raised when the Gutenberg download URL fails or returns no usable content."""

    def __init__(self, gutenberg_id: int):
        self.gutenberg_id = gutenberg_id
        super().__init__(f"Text unavailable for Gutenberg book #{gutenberg_id}")


class InvalidEpubError(Exception):
    """Raised when an uploaded file is not a parseable EPUB."""

    def __init__(self, reason: str):
        self.reason = reason
        super().__init__(f"Invalid EPUB: {reason}")


class BlobUploadError(Exception):
    """Raised when the upload to Vercel Blob storage fails."""

    def __init__(self, reason: str):
        self.reason = reason
        super().__init__(f"Blob upload failed: {reason}")


class UnsupportedFormatError(Exception):
    """Raised when no text or HTML format is present in the Gutendex formats dict."""

    def __init__(self, gutenberg_id: int, available_formats: list[str]):
        self.gutenberg_id = gutenberg_id
        self.available_formats = available_formats
        super().__init__(
            f"No supported text format for book #{gutenberg_id}. Available: {available_formats}"
        )

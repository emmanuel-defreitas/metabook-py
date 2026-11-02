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


class GutendexUnavailableError(Exception):
    """Raised when the Gutendex API cannot be reached (timeout, connection
    failure, or a non-2xx response from Gutendex itself)."""

    def __init__(self, reason: str, *, timed_out: bool = False):
        self.reason = reason
        self.timed_out = timed_out
        super().__init__(f"Gutendex unreachable: {reason}")


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


class TokenizerNotFoundError(Exception):
    """Raised when a tokenizer name does not resolve to a usable Hugging Face
    tokenizer (unknown repository, gated repository, or no tokenizer file)."""

    def __init__(self, name: str, reason: str):
        self.name = name
        self.reason = reason
        super().__init__(f"Tokenizer '{name}' not found: {reason}")


class TokenizerUnavailableError(Exception):
    """Raised when a tokenizer exists (or may exist) but could not be fetched —
    a transient network or Hub failure on cold start, not a bad request."""

    def __init__(self, name: str, reason: str):
        self.name = name
        self.reason = reason
        super().__init__(f"Tokenizer '{name}' unavailable: {reason}")


class UnsupportedFormatError(Exception):
    """Raised when no text or HTML format is present in the Gutendex formats dict."""

    def __init__(self, gutenberg_id: int, available_formats: list[str]):
        self.gutenberg_id = gutenberg_id
        self.available_formats = available_formats
        super().__init__(
            f"No supported text format for book #{gutenberg_id}. Available: {available_formats}"
        )


# ── Single-document analysis ───────────────────────────────────────────────────


class DocumentTooLargeError(Exception):
    """Raised when an uploaded or fetched document exceeds the size limit."""

    def __init__(self, max_bytes: int):
        self.max_bytes = max_bytes
        super().__init__(f"Document exceeds {max_bytes} bytes")


class UnsupportedDocumentTypeError(Exception):
    """Raised when a document's content type / extension is not accepted."""

    def __init__(self, content_type: str | None, allowed: list[str]):
        self.content_type = content_type
        self.allowed = allowed
        super().__init__(f"Unsupported document type: {content_type!r}")


class InvalidDocumentError(Exception):
    """Raised when a document of an accepted type cannot be parsed."""

    def __init__(self, reason: str):
        self.reason = reason
        super().__init__(f"Invalid document: {reason}")


class UrlNotAllowedError(Exception):
    """Raised when a URL fails validation (scheme, or resolves to a non-public address)."""

    def __init__(self, reason: str):
        self.reason = reason
        super().__init__(f"URL not allowed: {reason}")


class DocumentFetchError(Exception):
    """Raised when fetching a document URL fails or times out."""

    def __init__(self, reason: str, *, timed_out: bool = False):
        self.reason = reason
        self.timed_out = timed_out
        super().__init__(f"Document fetch failed: {reason}")

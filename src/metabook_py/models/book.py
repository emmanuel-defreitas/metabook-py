from pydantic import BaseModel, Field


class AuthorInfo(BaseModel):
    name: str
    birth_year: int | None = None
    death_year: int | None = None


class BookInfo(BaseModel):
    gutenberg_id: int
    title: str
    authors: list[AuthorInfo] = Field(default_factory=list)
    language: str = "en"
    subjects: list[str] = Field(default_factory=list)


class UploadedBookInfo(BaseModel):
    """Metadata extracted from an uploaded EPUB (no Gutenberg ID)."""

    source: str = "upload"
    title: str
    authors: list[AuthorInfo] = Field(default_factory=list)
    language: str = "en"
    subjects: list[str] = Field(default_factory=list)
    isbn: str | None = None


class BlobInfo(BaseModel):
    """Where the uploaded EPUB now lives in Vercel Blob storage."""

    url: str
    pathname: str
    size_bytes: int


class BookMatch(BaseModel):
    """Lightweight summary used in disambiguation lists."""

    gutenberg_id: int
    title: str
    authors: list[str]
    language: str


class DisambiguationResult(BaseModel):
    status: int = 300
    message: str = "Multiple books matched. Retry with a specific gutenberg_id."
    matches: list[BookMatch]


class SchemaInfo(BaseModel):
    name: str
    description: str
    hierarchy: list[str]

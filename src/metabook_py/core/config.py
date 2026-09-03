from pydantic_settings import BaseSettings, SettingsConfigDict


class Settings(BaseSettings):
    model_config = SettingsConfigDict(
        # later files win, so .env.local (written by `vercel env pull`) overrides .env
        env_file=(".env", ".env.local"),
        env_file_encoding="utf-8",
        # .env.local also carries vars we don't model (e.g. VERCEL_OIDC_TOKEN)
        extra="ignore",
    )

    gutendex_base_url: str = "https://gutendex.com"
    cache_ttl: int = 86400  # 24 hours in seconds
    rate_limit_per_second: float = 1.0
    max_disambiguations: int = 10
    blob_read_write_token: str = ""  # Vercel Blob RW token (BLOB_READ_WRITE_TOKEN)
    blob_api_url: str = "https://blob.vercel-storage.com"
    blob_folder: str = "books"
    blob_access: str = "private"  # must match the connected store's access mode
    max_upload_bytes: int = 50 * 1024 * 1024  # 50 MB
    # MongoDB persistence for the "uploads" collection (one document per book
    # a user uploaded or selected from search results). Empty URI = disabled;
    # every endpoint then behaves exactly as before.
    mongodb_uri: str = ""  # e.g. mongodb://localhost:27017 (MONGODB_URI)
    mongodb_db: str = "metabook"
    app_name: str = "Book Structure API"
    debug: bool = False


settings = Settings()

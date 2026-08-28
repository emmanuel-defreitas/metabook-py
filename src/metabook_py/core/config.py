from pydantic_settings import BaseSettings, SettingsConfigDict


class Settings(BaseSettings):
    model_config = SettingsConfigDict(env_file=".env", env_file_encoding="utf-8")

    gutendex_base_url: str = "https://gutendex.com"
    cache_ttl: int = 86400  # 24 hours in seconds
    rate_limit_per_second: float = 1.0
    max_disambiguations: int = 10
    app_name: str = "Book Structure API"
    debug: bool = False


settings = Settings()

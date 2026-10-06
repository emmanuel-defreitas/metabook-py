from __future__ import annotations

from dataclasses import dataclass

from .errors import ParseError


@dataclass(frozen=True, slots=True)
class Reference:
    corpus_id: str | None
    version: str | None
    sections: tuple[str, ...]
    otype: str | None = None
    start: int | None = None
    end: int | None = None

    def __post_init__(self) -> None:
        if not self.sections:
            raise ParseError("", "at least one section is required")
        if self.version is not None and self.corpus_id is None:
            raise ParseError(self.version, "a version requires a corpus id")
        for label, value in (("corpus id", self.corpus_id), ("version", self.version)):
            if value is not None and (not value or any(char in value for char in "/:@!")):
                raise ParseError(value or "", f"{label} contains a reserved delimiter")
        if self.otype is None:
            if self.start is not None or self.end is not None:
                raise ParseError(str(self.start), "an index requires a unit type")
            return
        if self.start is None or self.start < 1:
            raise ParseError(str(self.start), "selector indices are 1-based")
        if self.end is not None and self.end < self.start:
            raise ParseError(f"{self.start}-{self.end}", "range end precedes its start")

    @property
    def is_range(self) -> bool:
        return self.end is not None

    @property
    def is_section(self) -> bool:
        return self.otype is None

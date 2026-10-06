class RefError(Exception):
    """Base error for invalid or unresolvable references."""


class ParseError(RefError):
    def __init__(self, fragment: str, reason: str):
        self.fragment = fragment
        super().__init__(f"Invalid reference fragment {fragment!r}: {reason}")


class SectionNotFound(RefError):  # noqa: N818 - public contract
    def __init__(self, heading: str, level: str):
        self.heading = heading
        self.level = level
        super().__init__(f"Section {heading!r} was not found at level {level!r}")


class TypeNotInSection(RefError):  # noqa: N818 - public contract
    def __init__(self, otype: str, section: str):
        self.otype = otype
        self.section = section
        super().__init__(f"Type {otype!r} does not occur in section {section!r}")


class IndexOutOfRange(RefError):  # noqa: N818 - public contract
    def __init__(self, requested: str, count: int):
        self.requested = requested
        self.count = count
        super().__init__(f"Index {requested!r} is out of range; valid range is 1..{count}")

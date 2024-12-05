def test_import() -> None:
    from comsrv import ByteStreamInstrument  # noqa: F401 # type: ignore
    from comsrv import CanBus  # noqa: F401 # type: ignore


if __name__ == "__main__":
    test_import()

# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [2.4.0] - 2024-09-08

- Make HTTP endpoint optional (deprecated, to be removed in the future)
- Build and publish a docker container
- Correctly handle SIGTERM to shutdown application
- Add helm chart

## [2.3.1] - 2024-05-07

### Added

- Compile a C library for embedding in other application
- Cleanup/bump rust dependencies

## [2.3.0] - 2024-04-23

### Added

- Python type annotations
- Minor API cleanups

### Fixes

- Ensure timeouts are applied for `SerialScpiPipe`

## [2.2.0] - 2023-08-15

### Added

- Improve python project tooling by adding poetry and pyprojectx
- A full-duplex COBS stream to support communication with devices that use COBS for framing

### Fixed

- Various Dart functionality
- CAN message validation

## [2.1.1] - 2022-09-06

### Fixed

- Fix installation of python wheel on windows

## [2.1.0] - 2022-08-30

### Added

- Bump `async-can`, thus adding support for the `USR-CANET200` protocol

### Internal

- Clippy improvements
- Documentation improvements

## [2.0.0] - 2022-08-18

Initial public release

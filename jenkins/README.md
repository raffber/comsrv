# Continuous Integration Utilities

This directory provides a few utilities to test and build `poke`-utilities using docker.
The docker image is a based on ubuntu 20.04 and the generated binaries should thus be compatible with everything
newer than ubuntu 20.04.

Build the base docker image:

```shell
$ docker build . -t comsrv-agent
```

## Building the comsrv

Then, build the `comsrv`:

```shell
$ ./comsrv-build.sh
```

This will build the `comsrv` and place the resulting binary in `../comsrv/target/release`.


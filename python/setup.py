import setuptools
from os.path import dirname, join
from subprocess import check_output

curdir = dirname(__file__)
version = check_output([join(curdir, "..", "ci", "get-version.sh")]).decode().strip()

with open("../README.md") as f:
    long_description = f.read()

requirements = [
    "aiohttp~=3.7",
    "numpy~=1.22",
    "wsrpc @ git+https://github.com/raffber/wsrpc.git@release/v1.0.0#egg=wsrpc",
]


setuptools.setup(
    name="comsrv",
    version=version,
    author="Raphael Bernhard",
    author_email="beraphae@gmail.com",
    description="A communication relay for interfacing with instruments",
    long_description=long_description,
    long_description_content_type="text/markdown",
    url="https://github.com/raffber/comsrv",
    packages=["comsrv"],
    classifiers=[
        "Programming Language :: Python :: 3",
        "License :: OSI Approved :: MIT License",
        "Operating System :: OS Independent",
    ],
    python_requires=">=3.6",
    install_requires=requirements,
)

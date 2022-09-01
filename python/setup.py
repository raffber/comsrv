import setuptools
from os.path import dirname, join
from subprocess import check_output
import re

curdir = dirname(__file__)
with open(join(curdir, "..", "comsrv", "Cargo.toml")) as f:
    toml_file = f.read()

for version in re.finditer(r'version\s*\=\s*"(?P<version>.*?)"', toml_file):
    version = version.group("version")
    break

with open("../README.md") as f:
    long_description = f.read()

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
    install_requires=[
        "broadcast_wsrpc @ git+https://github.com/raffber/wsrpc.git@f0a63fcfb680c93439ce8e8352143473f3bfa765#egg=broadcast_wsrpc&subdirectory=python",
        "aiohttp~=3.7",
        "numpy~=1.22",
    ],
)

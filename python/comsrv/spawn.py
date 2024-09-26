import asyncio
import platform
from comsrv import ComSrv, HttpRpc
import tempfile
from os.path import join as join_path, exists as file_exists
import os
import aiohttp
import subprocess

VERSION = "2.5.0"
ON_WINDOWS = platform.system() == "Windows"
if ON_WINDOWS:
    COMSRV_BINARY = "comsrv.exe"
    COMSRV_URL = f"https://github.com/raffber/comsrv/releases/download/release%2F{VERSION}/comsrv.exe"
else:
    COMSRV_BINARY = "comsrv"
    COMSRV_URL = f"https://github.com/raffber/comsrv/releases/download/release%2F{VERSION}/comsrv"


async def spawn(rpc: None | HttpRpc = None) -> HttpRpc:
    if rpc is None:
        rpc = HttpRpc()
    running = await check_if_running(rpc=rpc)
    if running:
        return rpc
    tempdir = tempfile.gettempdir()
    comsrv_path = join_path(tempdir, COMSRV_BINARY)
    if not file_exists(comsrv_path):
        await download_file(COMSRV_URL, comsrv_path)
        if not ON_WINDOWS:
            os.chmod(comsrv_path, 0o755)
    run_comsrv_process(comsrv_path)
    rpc = HttpRpc("http://127.0.0.1:5903")
    for _ in range(100):
        running = await check_if_running(rpc=rpc)
        if running:
            return rpc
        await asyncio.sleep(0.1)
    raise Exception("Failed to start comsrv")


async def check_if_running(rpc: None | HttpRpc = None) -> bool:
    if rpc is None:
        rpc = HttpRpc()
    comsrv = ComSrv(rpc=rpc, timeout=0.2)
    try:
        await comsrv.get_version()
        return True
    except Exception:
        return False


def run_comsrv_process(path: str) -> subprocess.Popen:
    print("Running: ", path)
    if not ON_WINDOWS:
        return subprocess.Popen(
            [path],
            start_new_session=True,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
    return subprocess.Popen(
        [path],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )


async def download_file(url: str, path: str) -> None:
    async with aiohttp.ClientSession() as session:
        async with session.get(url) as response:
            with open(path, "wb") as file:
                while True:
                    chunk = await response.content.read(1024)
                    if not chunk:
                        break
                    file.write(chunk)

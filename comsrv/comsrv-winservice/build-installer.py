from os.path import join, abspath, dirname
from subprocess import call
import os

curdir = dirname(abspath(__file__))
os.chdir(curdir)

wix_path = r'C:\Program Files (x86)\WiX Toolset v3.11\bin'

candle = join(wix_path, 'candle')
light = join(wix_path, 'light')

call([candle, 'comsrv.wix'])
call([light, 'comsrv.wixobj'])

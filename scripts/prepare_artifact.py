import platform
import sys
from zipfile import ZipFile

target = sys.argv[1]

if platform.system() == 'Windows': 
  paths = ['cargo-flowistry.exe', 'flowistry-driver.exe']
else:
  paths = ['cargo-flowistry', 'flowistry-driver']

with ZipFile(f'{target}.zip', 'w') as f:
  for p in paths:
    f.write(p)
  
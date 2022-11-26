import subprocess as sp
import toml
import json

js_pkg_version = json.load(open('./ide/package.json', 'r'))['version']
cli_version = sp.check_output('cargo flowistry -V', shell=True).decode('utf-8').strip()

print('JS package version: ', js_pkg_version)
print('CLI version: ', cli_version)

assert js_pkg_version == cli_version

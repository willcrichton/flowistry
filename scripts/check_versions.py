import subprocess as sp
import toml
import json

rust_pkg_version = toml.load(open('./Cargo.toml', 'r'))['package']['version']
js_pkg_version = json.load(open('./ide/package.json', 'r'))['version']
cli_version = sp.check_output('cargo flowistry -V', shell=True).decode('utf-8').split(' ')[1].strip()

print('Rust package version: ', rust_pkg_version)
print('JS package version: ', js_pkg_version)
print('CLI version: ', cli_version)

assert rust_pkg_version == js_pkg_version and js_pkg_version == cli_version

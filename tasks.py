import re
import shutil
import zipfile
from pathlib import Path

import tomli
from invoke import task

ROOT = Path(__file__).parent
DIST = ROOT / "dist"


def get_version() -> str:
    manifest = ROOT / "Cargo.toml"
    return tomli.loads(manifest.read_bytes().decode("utf-8"))["package"]["version"]


@task
def clean(ctx):
    if DIST.exists():
        shutil.rmtree(DIST, ignore_errors=True)
    DIST.mkdir()


@task
def legal(ctx):
    version = get_version()
    txt_name = f"shawl-v{version}-legal.txt"
    txt_path = DIST / txt_name
    try:
        ctx.run(f'cargo lichking bundle --file "{txt_path}"', hide=True)
    except Exception:
        pass
    raw = txt_path.read_text("utf8")
    normalized = re.sub(r"C:\\Users\\[^\\]+", "~", raw)
    txt_path.write_text(normalized, "utf8")

    zip_path = DIST / f"shawl-v{version}-legal.zip"
    with zipfile.ZipFile(zip_path, "w", zipfile.ZIP_DEFLATED) as zip:
        zip.write(txt_path, txt_name)


@task
def docs(ctx):
    docs = Path(__file__).parent / "docs"
    cli = docs / "cli.md"

    commands = [
        "--help",
        "add --help",
        "run --help",
    ]

    lines = [
        "This is the raw help text for the command line interface.",
    ]
    for command in commands:
        output = ctx.run(f"cargo run -- {command}")
        lines.append("")
        lines.append(f"## `{command}`")
        lines.append("```")
        for line in output.stdout.splitlines():
            lines.append(line.rstrip())
        lines.append("```")

    if not docs.exists():
        docs.mkdir()
    cli.unlink()
    with cli.open("a") as f:
        for line in lines:
            f.write(line + "\n")


@task
def prerelease(ctx):
    # Make sure that the lock file has the new version
    ctx.run("cargo build")

    clean(ctx)
    legal(ctx)
    docs(ctx)


@task
def release(ctx):
    version = get_version()
    ctx.run(f'git commit -m "Release v{version}"')
    ctx.run(f'git tag v{version} -m "Release"')
    ctx.run("git push")
    ctx.run("git push --tags")


@task
def release_winget(ctx, target="/git/_forks/winget-pkgs"):
    target = Path(target)
    version = get_version()

    with ctx.cd(target):
        ctx.run("git checkout master")
        ctx.run("git pull upstream master")
        ctx.run(f"git checkout -b mtkennerly.shawl-{version}")
        ctx.run(f"wingetcreate update mtkennerly.shawl --version {version} --urls https://github.com/mtkennerly/shawl/releases/download/v{version}/shawl-v{version}-win64.zip https://github.com/mtkennerly/shawl/releases/download/v{version}/shawl-v{version}-win32.zip")
        ctx.run(f"code --wait manifests/m/mtkennerly/shawl/{version}/mtkennerly.shawl.locale.en-US.yaml")
        ctx.run(f"winget validate --manifest manifests/m/mtkennerly/shawl/{version}")
        ctx.run("git add .")
        ctx.run(f'git commit -m "mtkennerly.shawl version {version}"')
        ctx.run("git push origin HEAD")

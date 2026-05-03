import datetime as dt
import re
import shutil
import sys
import textwrap
import zipfile
from pathlib import Path

import tomli
from invoke import task

ROOT = Path(__file__).parent
DIST = ROOT / "dist"


def get_version() -> str:
    manifest = ROOT / "Cargo.toml"
    return tomli.loads(manifest.read_bytes().decode("utf-8"))["package"]["version"]


def replace_pattern_in_file(file: Path, old: str, new: str, count: int = 1):
    content = file.read_text("utf-8")
    updated = re.sub(old, new, content, count=count)
    file.write_text(updated, "utf-8")


def confirm(prompt: str):
    response = input(f"Confirm by typing '{prompt}': ")
    if response.lower() != prompt.lower():
        sys.exit(1)


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
def prerelease(ctx, new_version):
    date = dt.datetime.now().strftime("%Y-%m-%d")

    replace_pattern_in_file(
        ROOT / "Cargo.toml",
        'version = ".+"',
        f'version = "{new_version}"',
    )

    replace_pattern_in_file(
        ROOT / "CHANGELOG.md",
        "## Unreleased",
        f"## v{new_version} ({date})",
    )

    # Make sure that the lock file has the new version
    ctx.run("cargo build")

    clean(ctx)
    legal(ctx)
    docs(ctx)


@task
def release(ctx):
    version = get_version()

    confirm(f"release {version}")

    ctx.run(f'git commit -m "Release v{version}"')
    ctx.run(f'git tag v{version} -m "Release"')
    ctx.run("git push")
    ctx.run(f"git push origin tag v{version}")


@task
def release_winget(ctx, target="/dev/_forks/winget-pkgs"):
    target = Path(target)
    version = get_version()
    changelog = textwrap.indent(latest_changelog(), "  ")

    with ctx.cd(target):
        ctx.run("git checkout master")
        ctx.run("git pull upstream master")
        ctx.run(f"git checkout -b mtkennerly.shawl-{version}")
        ctx.run(f"wingetcreate update mtkennerly.shawl --version {version} --urls https://github.com/mtkennerly/shawl/releases/download/v{version}/shawl-v{version}-win64.zip https://github.com/mtkennerly/shawl/releases/download/v{version}/shawl-v{version}-win32.zip")

        spec = target / f"manifests/m/mtkennerly/shawl/{version}/mtkennerly.shawl.locale.en-US.yaml"
        spec_content = spec.read_bytes().decode("utf-8")
        spec_content = spec_content.replace("Moniker: shawl", f"Moniker: shawl\nReleaseNotes: |-\n{changelog}\nReleaseNotesUrl: https://github.com/mtkennerly/shawl/releases/tag/v{version}")
        spec.write_bytes(spec_content.encode("utf-8"))

        ctx.run(f"winget validate --manifest manifests/m/mtkennerly/shawl/{version}")
        ctx.run("git add .")
        ctx.run(f'git commit -m "mtkennerly.shawl version {version}"')
        ctx.run("git push origin HEAD")


def latest_changelog() -> str:
    changelog = ROOT / "CHANGELOG.md"
    content = changelog.read_bytes().decode("utf-8")

    lines = []
    header = False
    for line in content.splitlines():
        if line.startswith("#"):
            if header:
                break
            header = True
            continue

        lines.append(line)

    return "\n".join(lines).strip()

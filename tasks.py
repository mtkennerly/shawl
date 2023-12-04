import shutil
from pathlib import Path

import tomli
from invoke import task


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
def release(ctx):
    dist = Path("dist")
    if dist.exists():
        shutil.rmtree(dist)
    dist.mkdir()

    # Make sure that the lock file has the new version
    ctx.run("cargo build")

    docs(ctx)

    manifest = Path("Cargo.toml")
    version = tomli.loads(manifest.read_bytes().decode("utf-8"))["package"]["version"]
    ctx.run(f"cargo lichking bundle --file dist/shawl-v{version}-legal.txt")

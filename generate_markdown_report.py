import json
import re
import sys

with open(sys.argv[1]) as f:
    report_json = json.load(f)
with open(sys.argv[2]) as f:
    report_md = f.read()
base = sys.argv[3]

aliases = {m[1]: m[2].split(", ") for m in re.finditer(r"<li>([^ ]*) \((.*)\)</li>", report_md)}


def html_pkgs_section(emoji: str, packages: list[str], msg: str, what: str = "package") -> str:
    if len(packages) == 0:
        return ""
    plural = "s" if len(packages) > 1 else ""
    res = "<details>\n"
    res += f"  <summary>{emoji} {len(packages)} {what}{plural} {msg}:</summary>\n  <ul>\n"
    for pkg in packages:
        if pkg in aliases:
            pkg += f" ({", ".join(aliases[pkg])})"
        res += f"    <li>{pkg}</li>\n"
    res += "  </ul>\n</details>\n"
    return res


msg = ""
for system, report in report_json["result"].items():
    msg += "\n---\n"
    msg += f"### `{system}`\n"
    msg += html_pkgs_section(":fast_forward:", report["broken"], "marked as broken and skipped")
    msg += html_pkgs_section(
        ":fast_forward:", report["non-existent"], "present in ofBorgs evaluation, but not found in the checkout"
    )
    msg += html_pkgs_section(":fast_forward:", report["blacklisted"], "blacklisted")
    msg += html_pkgs_section(":x:", report["failed"], "failed to build")
    msg += html_pkgs_section(":x:", report.get("still_failing", []), f"still failing to build (also failed on {base})")
    msg += html_pkgs_section(":white_check_mark:", report["tests"], "built", what="test")
    msg += html_pkgs_section(":white_check_mark:", report["built"], "built")
    msg += html_pkgs_section(":grey_question:", report.get("unsupported", []), "not supported by the current system")

print(msg, end="")

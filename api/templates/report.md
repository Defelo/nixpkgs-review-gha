## {% if success %}:white_check_mark:{% else %}:x:{% endif %} `nixpkgs-review` result

Generated using [`nixpkgs-review-gha`](https://github.com/Defelo/nixpkgs-review-gha) ([`{{ workflow_sha | truncate(length=7, end="") }}`](https://github.com/Defelo/nixpkgs-review-gha/commit/{{ workflow_sha }}))

Command: `nixpkgs-review pr {{ pr }}{% if extra_args != '' %} {{ extra_args }}{% endif %}`
Commit: [`{{ head }}`](https://github.com/NixOS/nixpkgs/commit/{{ head }}) ([subsequent changes](https://github.com/NixOS/nixpkgs/compare/{{ head }}..pull/{{ pr }}/head))
Merge: [`{{ merge }}`](https://github.com/NixOS/nixpkgs/commit/{{ merge }})

Triggered by @{{ actor }} ({{ actor_id }})
Logs: {{ logs_url }}

{% macro fetch_cmds() -%}
{%- for x in systems -%}
{%- if not x.fetch_cmd %}{% continue %}{% endif -%}
<li><details><summary><code>{{ x.system }}</code></summary>

```shell
{{ x.fetch_cmd }}
```

</details></li>
{% endfor %}
{%- endmacro fetch_cmds -%}
{% if self::fetch_cmds() -%}
<details><summary>Download packages from cache:</summary><ul>
{{ self::fetch_cmds() -}}
</ul></details>
{%- endif %}

{%- macro pkgs_section(icon, attrs, msg, kind="package") %}
{%- if attrs | length != 0 -%}
<details><summary>{{ icon }} {{ attrs | length }} {{ kind }}{{ attrs | length | pluralize }} {{ msg }}:</summary><ul>
{%- for attr in attrs %}
<li>{{ attr.name }}{% if attr.aliases | length != 0 %} ({{ attr.aliases | join(sep=", ") }}){% endif %}</li>
{%- endfor %}
</ul></details>
{%- endif -%}
{% endmacro pkgs_section -%}
{% for x in systems %}

---
### `{{ x.system }}`{% if x.system is not matching("-linux$") and system_has_rebuilds(report=x) %} (sandbox = {{ x.sandbox }}){% endif %}
{{ self::pkgs_section(icon=":fast_forward:", attrs=x.broken, msg="marked as broken and skipped") -}}
{{ self::pkgs_section(icon=":fast_forward:", attrs=x.non_existent, msg="present in ofBorgs evaluation, but not found in the checkout") -}}
{{ self::pkgs_section(icon=":fast_forward:", attrs=x.blacklisted, msg="blacklisted") -}}
{{ self::pkgs_section(icon=":x:", attrs=x.failed, msg="failed to build") -}}
{{ self::pkgs_section(icon=":x:", attrs=x.still_failing, msg="still failing to build (also failed on " ~ base_ref ~ ")") -}}
{{ self::pkgs_section(icon=":white_check_mark:", attrs=x.tests, msg="built", kind="test") -}}
{{ self::pkgs_section(icon=":white_check_mark:", attrs=x.built, msg="built") -}}
{{ self::pkgs_section(icon=":grey_question:", attrs=x.unsupported, msg="not supported on this runner") -}}
{% if not system_has_rebuilds(report=x) -%}
:white_check_mark: *No rebuilds*
{%- endif -%}

{% endfor -%}

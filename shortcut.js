// ==UserScript==
// @name        nixpkgs-review-gha
// @match       https://github.com/*
// @run-at      document-idle
// ==/UserScript==

const repo = "Defelo/nixpkgs-review-gha";

const reviewDefaults = ({
  title,
  commits,
  labels,
  author,
  authoredByMe,
  hasLinuxRebuilds,
  hasDarwinRebuilds,
  state,
}) => {
  const darwinSandboxFalseList = ["miniserve"];
  const x8664DarwinSkipList = [];

  const testMap = [[["radicle-ci-broker", "radicle-native-ci", "radicle-job"], "nixosTests.radicle-ci-broker"]];
  const testPackages = [
    "alertmanager-ntfy",
    "anubis",
    "chhoto-url",
    "echoip",
    "glitchtip",
    "go-httpbin",
    "olivetin",
    "privatebin",
    "radicle-httpd",
    "radicle-node",
    "taplo",
    "uiua",
    "uiua-unstable",
    "whoami",
    "zipline",
  ];

  const skipMap = [[["taplo"], "servo"]];

  const pkgsChanged = pkgs => pkgs.some(p => commits.some(({ subject }) => subject.startsWith(`${p}:`)));

  const extraPkgs = new Set(
    testMap
      .filter(([pkgs]) => pkgsChanged(pkgs))
      .map(([, test]) => test)
      .concat(testPackages.filter(pkg => pkgsChanged([pkg])).map(pkg => `${pkg}.tests`)),
  );

  const skipPkgs = new Set(
    skipMap.filter(([pkgs, skip]) => pkgsChanged(pkgs) && !pkgsChanged([skip])).map(([, skip]) => skip),
  );

  const darwinSandbox = pkgsChanged(darwinSandboxFalseList) ? "false" : "relaxed";
  const hasRebuilds = hasLinuxRebuilds || hasDarwinRebuilds;

  return {
    branch: "private",
    "x86_64-linux": !hasRebuilds || hasLinuxRebuilds,
    "aarch64-linux": !hasRebuilds || hasLinuxRebuilds,
    "x86_64-darwin":
      !hasRebuilds || (hasDarwinRebuilds && !pkgsChanged(x8664DarwinSkipList)) ? `yes_sandbox_${darwinSandbox}` : "no",
    "aarch64-darwin": !hasRebuilds || hasDarwinRebuilds ? `yes_sandbox_${darwinSandbox}` : "no",
    "extra-args": [...extraPkgs]
      .map(pkg => `-a ${pkg}`)
      .concat([...skipPkgs].map(pkg => `--skip-package ${pkg}`))
      .join(" "),
    // "push-to-cache": true,
    // "upterm": false,
    // "post-result": true,
    "on-success": state === "DRAFT" ? "mark_as_ready" : "nothing",
  };
};

const prTrackers = [
  { name: "nixpk.gs", toUrl: pr => `https://nixpk.gs/pr-tracker.html?pr=${pr}` },
  { name: "ocfox.me", toUrl: pr => `https://nixpkgs-tracker.ocfox.me/?pr=${pr}` },
];

const sleep = duration => new Promise(resolve => setTimeout(resolve, duration));
const query = async (doc, sel) => {
  await sleep(0);
  while (true) {
    const elem = doc.querySelector(sel);
    if (elem !== null) return elem;
    await sleep(100);
  }
};

const getPrDetails = pr => {
  const title = document.querySelector("bdi.js-issue-title.markdown-title").innerText;
  const commits = [...document.querySelectorAll(".TimelineItem-body a.markdown-title[href*='/commits/']")].flatMap(
    ({ title, href }) => {
      const match = /\/NixOS\/nixpkgs\/pull\/(\d+)\/commits\/([0-9a-f]+)$/i.exec(href);
      return match === null || match[1] !== pr
        ? []
        : [
            {
              commit_id: match[2],
              subject: title.split("\n")[0],
              description: title,
            },
          ];
    },
  );
  const labels = [...document.querySelectorAll("div.js-issue-labels > a")].map(x => x.innerText);
  const author = document.querySelector(".js-discussion > :first-child a.author").href.split("/").at(-1);
  const self = document.querySelector("header.GlobalNav button[data-login]").getAttribute("data-login");
  const authoredByMe = author === self;
  const hasLinuxRebuilds = !labels.some(l => /rebuild-linux: 0$/.test(l));
  const hasDarwinRebuilds = !labels.some(l => /rebuild-darwin: 0$/.test(l));
  const state = document.querySelector("span.State").innerText.trim().toUpperCase();

  return { title, commits, labels, author, authoredByMe, hasLinuxRebuilds, hasDarwinRebuilds, state };
};

const setupActionsPage = async () => {
  const match = /^https:\/\/github.com\/([^/]+\/[^/]+)\/actions\/workflows\/review.yml#dispatch:(.*)$/.exec(
    location.href,
  );
  if (match === null || match[1] !== repo) return;

  const inputs = new URLSearchParams(match[2]);

  (await query(document, "details > summary.btn")).click();
  await query(document, "details .workflow-dispatch");

  const setBranch = async branch => {
    (await query(document, "details .workflow-dispatch")).classList.add("old-branch");
    document.querySelector("details .workflow-dispatch .branch-selection > details > summary").click();
    (await query(document, `ref-selector[type=branch] button[value="${branch}"]`)).click();
    while ((await query(document, "details .workflow-dispatch")).classList.contains("old-branch")) await sleep(100);
  };

  const setInput = (name, value) => {
    const selector = `details .workflow-dispatch [name='inputs[${name}]']`;
    const input = document.querySelector(`${selector}:not([type=hidden])`);

    if (!input) {
      alert(`workflow_dispatch input '${name}' does not exist`);
      return;
    }

    if (input.type === "checkbox") {
      if (!["true", "false"].includes(value)) {
        alert(`workflow_dispatch input '${name}' expects a boolean ('true' or 'false) but is set to '${value}'`);
        return;
      }
      input.checked = value === "true";
    } else {
      input.value = value;
    }
  };

  const branch = inputs.get("branch");
  if (branch) await setBranch(branch);

  [...inputs].filter(([name]) => name !== "branch").forEach(([name, value]) => setInput(name, value));

  document.querySelector("details .workflow-dispatch button[type=submit]").focus();
};

const setupPrPage = async () => {
  const match = /^https:\/\/github.com\/NixOS\/nixpkgs\/pull\/(\d+)([?#].*)?$/i.exec(location.href);
  if (match === null) return;

  const pr = match[1];
  const actions = await query(document, ".gh-header-show .gh-header-actions");

  if (actions.querySelector(".run-nixpkgs-review") === null) {
    const btn = document.createElement("button");
    btn.classList.add("Button", "Button--secondary", "Button--small", "run-nixpkgs-review");
    btn.innerText = "Run nixpkgs-review";
    actions.prepend(btn);
    btn.onclick = () => {
      const params = new URLSearchParams({ ...reviewDefaults(getPrDetails(pr)), pr });
      window.open(`https://github.com/${repo}/actions/workflows/review.yml#dispatch:${params}`);
    };
  }

  const { hasLinuxRebuilds, hasDarwinRebuilds, state } = getPrDetails(pr);
  if ((!hasLinuxRebuilds && !hasDarwinRebuilds) || state == "MERGED") {
    actions.querySelector(".run-nixpkgs-review").setAttribute("aria-disabled", true);
  }

  if (actions.querySelector(".goto-pr-tracker") === null) {
    for (const { name, toUrl } of prTrackers) {
      const btn = document.createElement("button");
      btn.classList.add("Button", "Button--secondary", "Button--small", "goto-pr-tracker");
      btn.innerText = prTrackers.length === 1 ? "PR Tracker" : `PR Tracker (${name})`;
      actions.prepend(btn);
      btn.onclick = () => {
        window.open(toUrl(pr));
      };
    }
  }
};

new MutationObserver(setupPrPage).observe(document, { subtree: true, childList: true });

setupActionsPage();
setupPrPage();

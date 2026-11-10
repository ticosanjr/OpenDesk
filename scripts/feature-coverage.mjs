#!/usr/bin/env node
import { readFileSync } from "node:fs";
import { FEATURES } from "../tests/e2e/features.ts";

const reportPath = process.argv[2] ?? "test-results/features.json";
const threshold = Number(process.env.OPENDESK_FEATURE_THRESHOLD ?? 95);

let report;
try {
  report = JSON.parse(readFileSync(reportPath, "utf8"));
} catch (error) {
  console.error(`feature coverage: cannot read ${reportPath}: ${error.message}`);
  process.exit(1);
}

const specs = [];
function walk(suite) {
  for (const spec of suite.specs ?? []) {
    const statuses = (spec.tests ?? []).flatMap((test) =>
      (test.results ?? []).map((result) => result.status),
    );
    specs.push({
      file: spec.file ?? suite.file ?? "",
      title: spec.title ?? "",
      passed: statuses.length > 0 && statuses.every((status) => status === "passed"),
      statuses,
    });
  }
  for (const child of suite.suites ?? []) walk(child);
}
for (const suite of report.suites ?? []) walk(suite);

const missing = [];
for (const feature of FEATURES) {
  const found = specs.find(
    (spec) =>
      spec.file.endsWith(feature.spec) &&
      spec.title.includes(feature.test) &&
      spec.passed,
  );
  if (!found) missing.push(feature);
}

const covered = FEATURES.length - missing.length;
const percent = FEATURES.length === 0 ? 0 : (covered / FEATURES.length) * 100;
console.log(
  `feature coverage: ${covered}/${FEATURES.length} (${percent.toFixed(1)}%) — threshold ${threshold}%`,
);
if (missing.length > 0) {
  console.log("\nUncovered or failing features:");
  for (const feature of missing) {
    console.log(`- ${feature.id} [${feature.spec}] ${feature.test}`);
  }
}
if (percent < threshold) {
  console.error(`\nfeature coverage below threshold (${percent.toFixed(1)}% < ${threshold}%)`);
  process.exit(1);
}

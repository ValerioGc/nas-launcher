// Coverage of the antivirus implementation hosted by the launcher, with an 80% floor per file.
import { execFileSync } from 'node:child_process';
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';

const directory = 'backend/coverage';
mkdirSync(directory, { recursive: true });
const check = process.argv[2] === '--check';
const complete = check
  ? (process.argv[3] ?? `${directory}/lcov.info`)
  : `${directory}/antivirus-all.lcov`;
if (!check)
  execFileSync(
    'cargo',
    [
      'llvm-cov',
      '--manifest-path',
      'backend/Cargo.toml',
      '--lib',
      '--test',
      'machine',
      '--test',
      'health',
      '--lcov',
      '--ignore-filename-regex',
      'tests/',
      '--output-path',
      complete,
    ],
    { stdio: 'inherit' },
  );
const records = readFileSync(complete, 'utf8')
  .split('end_of_record')
  .filter((record) =>
    /^SF:.*\/src\/apps\/(controllers\/antivirus\.rs|services\/antivirus(?:\.rs|\/[^\n]+))$/m.test(
      record,
    ),
  );
if (records.length !== 6)
  throw new Error(`Expected all 6 antivirus sources, found ${records.length}`);
const summary = records.map((record) => {
  const file = record.match(/^SF:(.+)$/m)?.[1];
  const counts = Object.fromEntries(
    record
      .split('\n')
      .filter((line) => /^(LF|LH|FNF|FNH):/.test(line))
      .map((line) => {
        const [key, value] = line.split(':');
        return [key, Number(value)];
      }),
  );
  const lines = (100 * counts.LH) / counts.LF;
  const functions = (100 * counts.FNH) / counts.FNF;
  return { file, lines, functions };
});
writeFileSync(
  `${directory}/antivirus-lcov.info`,
  records.map((record) => `${record.trim()}\nend_of_record\n`).join(''),
);
writeFileSync(`${directory}/antivirus-summary.json`, `${JSON.stringify(summary, null, 2)}\n`);
for (const item of summary) {
  console.log(
    `${item.file}: lines ${item.lines.toFixed(2)}%, functions ${item.functions.toFixed(2)}%`,
  );
}
if (
  summary.some(
    (item) =>
      !Number.isFinite(item.lines) ||
      item.lines < 80 ||
      !Number.isFinite(item.functions) ||
      item.functions < 80,
  )
) {
  throw new Error('Antivirus coverage must reach 80% lines and functions in every file');
}

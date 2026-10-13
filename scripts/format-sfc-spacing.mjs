import { readdirSync, readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { format, resolveConfig } from 'prettier';

/**
 * Formatter of the Vue components: prettier, plus two rules that prettier cannot enforce.
 *
 * 1. One empty line immediately after the opening and before the closing of `<script>` and `<style>`.
 * 2. An empty line between two adjacent CSS rules.
 *
 * Prettier removes empty lines at the beginning of a block, and it has no option to keep them: for this reason,
 * `.vue` files are in `.prettierignore` and are processed here. The script calls prettier through its
 * API, so the SFCs remain formatted like everything else: with these two additional rules.
 *
 *   node scripts/format-sfc-spacing.mjs:          rewrites the files
 *   node scripts/format-sfc-spacing.mjs --check    only shows which ones need to be rewritten
 */

const checkOnly = process.argv.includes('--check');

function findVueFiles(directory) {
  return readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const path = join(directory, entry.name);

    if (entry.isDirectory()) {
      return findVueFiles(path);
    }

    return path.endsWith('.vue') ? [path] : [];
  });
}

function startsAnotherRule(next) {
  const line = (next ?? '').trim();

  return line !== '' && line !== '}' && !line.startsWith('@else') && !line.startsWith('</');
}

function applySpacing(source) {
  const lineEnding = source.includes('\r\n') ? '\r\n' : '\n';
  const lines = source.split(/\r?\n/);
  const formatted = [];
  let insideStyle = false;

  for (let index = 0; index < lines.length; index += 1) {
    const line = lines[index];
    const openingTag = /^\s*<(script|style)\b[^>]*>\s*$/.exec(line);
    const closingTag = /^\s*<\/(script|style)>\s*$/.test(line);

    if (openingTag) {
      insideStyle = openingTag[1] === 'style';
      formatted.push(line);

      while (index + 1 < lines.length && lines[index + 1].trim() === '') {
        index += 1;
      }

      formatted.push('');
      continue;
    }

    if (closingTag) {
      while (formatted.length > 0 && formatted.at(-1).trim() === '') {
        formatted.pop();
      }

      formatted.push('');
      formatted.push(line);
      insideStyle = false;
      continue;
    }

    formatted.push(line);

    if (insideStyle && line.trim() === '}' && startsAnotherRule(lines[index + 1])) {
      formatted.push('');
    }
  }

  return formatted.join(lineEnding);
}

/** How the file should look: first prettier, then the two rules. */
async function expected(file, source) {
  const options = await resolveConfig(file);

  return applySpacing(await format(source, { ...options, filepath: file }));
}

const pending = [];

for (const file of findVueFiles('src')) {
  const source = readFileSync(file, 'utf8');
  const wanted = await expected(file, source);

  if (wanted === source) {
    continue;
  }

  pending.push(file);

  if (!checkOnly) {
    writeFileSync(file, wanted);
  }
}

if (checkOnly && pending.length > 0) {
  console.error(`Componenti da formattare (${pending.length}):`);
  pending.forEach((file) => console.error(`  ${file}`));
  console.error('\nSistemali con:  pnpm run format:sfc');
  process.exit(1);
}

if (!checkOnly && pending.length > 0) {
  console.log(`Componenti riformattati: ${pending.length}`);
}

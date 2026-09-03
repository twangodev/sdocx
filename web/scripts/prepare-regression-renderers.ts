import { execFileSync } from 'node:child_process';
import {
	mkdtempSync,
	mkdirSync,
	rmSync,
	writeFileSync
} from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

interface PreparedRenderer {
	sha: string;
	refs: string[];
	directory: string;
}

const webDirectory = dirname(dirname(fileURLToPath(import.meta.url)));
const repository = join(webDirectory, '..');
const outputRoot = join(webDirectory, 'static', 'renderers');
const requestedRefs = process.argv.slice(2).filter((argument) => argument !== '--');

function command(executable: string, args: string[], cwd = repository): string {
	return execFileSync(executable, args, { cwd, encoding: 'utf8', stdio: ['ignore', 'pipe', 'inherit'] }).trim();
}

function resolveCommit(ref: string): string {
	if (!ref || ref.length > 80 || ref.includes('\0')) throw new Error(`Invalid Git ref: ${ref}`);
	return command('git', ['rev-parse', '--verify', '--end-of-options', `${ref}^{commit}`]).toLowerCase();
}

if (requestedRefs.length !== 2) {
	throw new Error('Usage: bun run regression:prepare -- <from-ref> <to-ref>');
}

const artifacts = new Map<string, PreparedRenderer>();
for (const ref of requestedRefs) {
	const sha = resolveCommit(ref);
	const existing = artifacts.get(sha);
	if (existing) existing.refs.push(ref);
	else {
		artifacts.set(sha, {
			sha,
			refs: [ref],
			directory: `/renderers/${sha}`
		});
	}
}

const temporaryRoot = mkdtempSync(join(tmpdir(), 'sdocx-renderers-'));
rmSync(outputRoot, { recursive: true, force: true });
mkdirSync(outputRoot, { recursive: true });

try {
	for (const artifact of artifacts.values()) {
		const worktree = join(temporaryRoot, artifact.sha.slice(0, 12));
		const output = join(outputRoot, artifact.sha);
		console.log(`building ${artifact.refs.join(', ')} (${artifact.sha.slice(0, 12)})`);
		command('git', ['worktree', 'add', '--detach', worktree, artifact.sha]);
		try {
			execFileSync(
				'wasm-pack',
				[
					'build',
					join(worktree, 'crates', 'sdocx-wasm'),
					'--target',
					'web',
					'--release',
					'--out-dir',
					output,
					'--out-name',
					'sdocx_wasm',
					'--no-typescript',
					'--no-pack'
				],
				{ cwd: repository, stdio: 'inherit' }
			);
		} finally {
			execFileSync('git', ['worktree', 'remove', '--force', worktree], {
				cwd: repository,
				stdio: 'inherit'
			});
		}
	}

	writeFileSync(
		join(outputRoot, 'manifest.json'),
		`${JSON.stringify(
			{
				version: 1,
				generatedAt: new Date().toISOString(),
				renderers: Array.from(artifacts.values())
			},
			null,
			2
		)}\n`
	);
	console.log(`prepared ${artifacts.size} renderer${artifacts.size === 1 ? '' : 's'}`);
} finally {
	rmSync(temporaryRoot, { recursive: true, force: true });
}

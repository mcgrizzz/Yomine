// Trims the directories common to every path so the tree root is meaningful,
// then builds a nested dir/file tree.

export interface TreeNode {
	/** Stable id (the full path for files, the joined dir path for directories). */
	id: string;
	/** Display segment (the last path component). */
	name: string;
	/** Absolute file path for leaves; `null` for directory nodes. */
	path: string | null;
	children: TreeNode[];
}

/** Split a path on both separators (paths may be Windows or POSIX). */
function splitPath(p: string): string[] {
	return p.split(/[/\\]/).filter((s) => s.length > 0);
}

/** Number of leading components shared by every path (the common ancestor). */
function commonAncestorDepth(parts: string[][]): number {
	if (parts.length === 0) return 0;
	const first = parts[0];
	let depth = 0;
	while (depth < first.length && parts.every((p) => p[depth] === first[depth])) {
		depth++;
	}
	return depth;
}

/**
 * Build a nested tree from a flat path list, trimming the common-ancestor dirs.
 * Directories sort before/with files alphabetically; leaves carry their full path.
 */
export function buildFileTree(paths: string[]): TreeNode[] {
	if (paths.length === 0) return [];

	const parts = paths.map(splitPath);
	const trim = commonAncestorDepth(parts);
	// Never trim the last (file-name) component, so single files keep a name.
	const display = Math.min(trim, Math.max(0, (parts[0]?.length ?? 1) - 1));

	const roots: TreeNode[] = [];

	paths.forEach((full, idx) => {
		const visible = parts[idx].slice(display);
		let level = roots;
		let prefix = '';
		visible.forEach((segment, i) => {
			const isLeaf = i === visible.length - 1;
			prefix = prefix ? `${prefix}/${segment}` : segment;
			if (isLeaf) {
				level.push({ id: full, name: segment, path: full, children: [] });
			} else {
				let dir = level.find((n) => n.path === null && n.name === segment);
				if (!dir) {
					dir = { id: `dir:${prefix}`, name: segment, path: null, children: [] };
					level.push(dir);
				}
				level = dir.children;
			}
		});
	});

	sortNodes(roots);
	return roots;
}

function sortNodes(nodes: TreeNode[]): void {
	nodes.sort((a, b) => {
		// Directories first, then alphabetical (mirrors egui's sort_children spirit).
		if ((a.path === null) !== (b.path === null)) return a.path === null ? -1 : 1;
		return a.name.localeCompare(b.name);
	});
	for (const n of nodes) sortNodes(n.children);
}

/** Collect the file paths under a node (recursively) matching `pred`. */
export function collectChecked(node: TreeNode, pred: (path: string) => boolean): string[] {
	const out: string[] = [];
	const walk = (n: TreeNode) => {
		if (n.path) {
			if (pred(n.path)) out.push(n.path);
		} else {
			n.children.forEach(walk);
		}
	};
	walk(node);
	return out;
}

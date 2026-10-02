#!/usr/bin/env python3
"""PRD-028 Phase 0: aggregate profile of candidate corpora held in visionGraph.

Emits counts only. No title, path, sentence or URL from a non-public stratum is
written out, so the result is safe to commit to a public repository.

Usage: profile_corpus.py <git root> [name=relative/dir ...] > corpus-profile.json

With no strata named, the four visionGraph strata are profiled. Naming strata
profiles those directories instead, so the same measures apply to any candidate
held in a git checkout.
"""
import hashlib
import json
import re
import statistics
import subprocess
import sys
from collections import Counter
from pathlib import Path

ROOT = Path(sys.argv[1]).resolve()
DEFAULT_STRATA = {
    'knowledge': ROOT / 'knowledge/pages',      # published OKF corpus: the regression reference
    'working-pages': ROOT / 'working/pages',    # curator notes
    'working-journals': ROOT / 'working/journals',
    'transcripts': ROOT / 'transcripts',        # third-party podcast and article transcripts
}
STRATA = (dict((k, ROOT / v) for k, v in (a.split('=', 1) for a in sys.argv[2:]))
          or DEFAULT_STRATA)
URL = re.compile(r'https?://\S+')
WIKILINK = re.compile(r'\[\[([^\]|#]+)')
DATE_NAME = re.compile(r'^(\d{4})-(\d{2})-\d{2}$')


def split(text):
    """Frontmatter (raw) and body. Frontmatter is the leading `---` block."""
    if text.startswith('---\n'):
        end = text.find('\n---', 4)
        if end != -1:
            return text[4:end], text[end + 4:]
    return '', text


def front_value(front, key):
    m = re.search(rf'^{re.escape(key)}:\s*(.*)$', front, re.M)
    return m.group(1).strip().strip('\'"') if m else None


def git_first_added(paths_root):
    """File-add events per commit year under a path (renames and re-imports count again)."""
    out = subprocess.run(
        ['git', '-C', str(ROOT), 'log', '--diff-filter=A', '--name-only', '--format=@%cs',
         '--', str(paths_root.relative_to(ROOT))],
        capture_output=True, text=True, check=True).stdout
    years, year = Counter(), None
    for line in out.splitlines():
        if line.startswith('@'):
            year = line[1:5]
        elif line.strip():
            years[year] += 1
    return dict(sorted(years.items()))


def pct(values, q):
    values = sorted(values)
    return values[min(len(values) - 1, int(q * len(values)))] if values else 0


def profile(name, directory):
    files = sorted(p for p in directory.rglob('*.md') if '/.deleted/' not in str(p))
    words, chars, body_hashes, line_counts = [], [], Counter(), Counter()
    url_lines = nonempty_lines = linked_units = links = stubs = 0
    public, status, types, domains, generated_by = Counter(), Counter(), Counter(), Counter(), Counter()
    journal_years = Counter()
    for path in files:
        text = path.read_text(encoding='utf-8', errors='replace')
        front, body = split(text)
        public[front_value(front, 'public') or 'absent'] += 1
        status[front_value(front, 'status') or 'absent'] += 1
        types[front_value(front, 'type') or 'absent'] += 1
        if (d := front_value(front, 'domain')):
            domains[d] += 1
        m = re.search(r'^generated:\s*\n\s+by:\s*(\S+)', front, re.M)
        generated_by[m.group(1) if m else 'absent'] += 1
        if (dm := DATE_NAME.match(path.stem)):
            journal_years[dm.group(1)] += 1
        w = len(body.split())
        words.append(w)
        chars.append(len(body))
        stubs += w < 50
        norm = ' '.join(body.lower().split())
        body_hashes[hashlib.sha256(norm.encode()).hexdigest()] += 1
        unit_links = len(WIKILINK.findall(text))
        links += unit_links
        linked_units += unit_links > 0
        for line in body.splitlines():
            s = line.strip().lstrip('-* ').strip()
            if not s:
                continue
            nonempty_lines += 1
            url_lines += bool(URL.search(s))
            if len(s) >= 40:
                line_counts[s.lower()] += 1
    n = len(files)
    dup_units = sum(c - 1 for c in body_hashes.values() if c > 1)
    long_lines = sum(line_counts.values())
    repeated_lines = sum(c for c in line_counts.values() if c > 1)
    return {
        'stratum': name,
        'units': n,
        'words': sum(words),
        'est_tokens_chars_div_4': sum(chars) // 4,
        'words_per_unit': {'median': int(statistics.median(words)) if words else 0,
                           'p90': pct(words, 0.9), 'max': max(words, default=0)},
        'stub_fraction_lt_50_words': round(stubs / n, 3) if n else 0,
        'exact_duplicate_unit_fraction': round(dup_units / n, 3) if n else 0,
        'repeated_long_line_fraction': round(repeated_lines / long_lines, 3) if long_lines else 0,
        'url_bearing_line_fraction': round(url_lines / nonempty_lines, 3) if nonempty_lines else 0,
        'wikilinks_per_unit': round(links / n, 2) if n else 0,
        'units_with_outbound_link_fraction': round(linked_units / n, 3) if n else 0,
        'frontmatter_public': dict(public.most_common()),
        'frontmatter_status': dict(status.most_common(6)),
        'frontmatter_type': dict(types.most_common(6)),
        'domains': dict(domains.most_common()),
        'generated_by': dict(generated_by.most_common(4)),
        'dated_filename_years': dict(sorted(journal_years.items())),
        'git_add_events_by_year': git_first_added(directory),
    }


head = subprocess.run(['git', '-C', str(ROOT), 'rev-parse', 'HEAD'],
                      capture_output=True, text=True, check=True).stdout.strip()
print(json.dumps({'corpus': f'{ROOT.name}@{head}',
                  'strata': [profile(k, v) for k, v in STRATA.items() if v.is_dir()]},
                 indent=1))

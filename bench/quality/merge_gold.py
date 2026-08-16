#!/usr/bin/env python3
"""Merge the 6 gold shards + the pilot gold into one findability-scored set."""
import json, glob, statistics, sys

merged = {}
for f in sorted(glob.glob('uplift-results/quality/gold-shard*.json')):
    for g in json.load(open(f)):
        merged[g['id']] = g
for g in json.load(open('uplift-results/quality/pilot-gold.json')):
    if g['id'] in merged:
        continue
    merged[g['id']] = {
        'id': g['id'],
        'stratum': 'private' if g.get('abstained') else 'public',
        'web_findability': 0.15 if g.get('abstained') else round(g.get('confidence', 0.6), 2),
        'answer': g.get('answer', ''),
        'cited_sources': g.get('cited_sources', []),
        'reference_source': 'corpus' if g.get('abstained') else 'web',
        'abstained': g.get('abstained', False),
        'confidence': g.get('confidence', 0.6),
        'notes': g.get('notes', ''),
    }
gs = list(merged.values())
json.dump(gs, open('uplift-results/quality/gold-merged.json', 'w'), indent=2)

pub = [g for g in gs if g['stratum'] == 'public']
priv = [g for g in gs if g['stratum'] == 'private']
absd = sum(1 for g in gs if g.get('abstained'))
fs = [g['web_findability'] for g in gs if isinstance(g.get('web_findability'), (int, float))]
bins = {'0.0-0.4': 0, '0.4-0.6': 0, '0.6-0.8': 0, '0.8-1.0': 0}
for x in fs:
    k = '0.0-0.4' if x < 0.4 else '0.4-0.6' if x < 0.6 else '0.6-0.8' if x < 0.8 else '0.8-1.0'
    bins[k] += 1
print('merged gold:', len(gs))
print('  public=%d private=%d abstained=%d corpus-authored=%d'
      % (len(pub), len(priv), absd, sum(1 for g in gs if g.get('reference_source') == 'corpus')))
print('  findability mean=%.2f min=%.2f max=%.2f' % (statistics.mean(fs), min(fs), max(fs)))
print('  findability bins:', bins)

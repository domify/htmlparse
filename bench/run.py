#!/usr/bin/env python3
"""Times htmlparse against html5ever 0.35 on the same documents.

Builds bench/parse.ri with `ris build --release` and bench/rust with cargo
(LTO, one codegen unit), then runs each on every workload: one warmup and
--rounds measured runs, in a shuffled order. Each run parses the input n
times, as a document or as a fragment in `body`, and serializes every
tree, timing the two apart; the checksums of the two outputs must match.

The memory a parse takes is the peak RSS of a process that parses the
input once, without serializing it, less that of one that only reads it:
whole pages, so it is reported for the inputs of 44 KiB and up.

Writes bench/results.json and prints Markdown tables.
"""
import argparse
import json
import os
import platform
import random
import statistics
import subprocess
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]

ROW = ('<tr class="r{i}"><td><a href="/item/{i}?a=1&amp;b=2">Item {i}</a>'
       '</td><td>{i}&nbsp;&euro;</td><td><input type=checkbox checked '
       'name="c{i}"></td></tr>\n')
ARTICLE = '''<article id="post-{i}" class="post entry" data-id="{i}">
<h2><a href="/posts/{i}" title="Post {i}">Post number {i} &mdash; õunad &amp; pirnid</a></h2>
<p class="meta">By <span class=author>Writer {i}</span> on <time datetime="2026-10-{d:02}">October {d}</time></p>
<p>Lorem ipsum dolor sit amet, <em>consectetur</em> adipiscing elit, sed do
<strong>eiusmod tempor</strong> incididunt ut labore et dolore magna aliqua.
Ut enim ad minim veniam, quis nostrud exercitation <code>ullamco &lt;b&gt;</code>
laboris nisi ut aliquip ex ea commodo consequat. &copy; &#169; &#x1F600;
<p>Unclosed paragraph with <b>bold <i>and italic</b> misnested</i> text,
an <img src="/img/{i}.png" alt="Image {i}" width=640 height=480> image
and a <br> break.
<ul><li>One<li>Two<li>Three <a href=#x>link</a></ul>
<!-- comment {i} -->
<svg width="24" height="24" viewBox="0 0 24 24"><path d="M12 2L2 7l10 5 10-5-10-5z"/><foreignObject><p>in svg</p></foreignObject></svg>
<table><caption>Table {i}</caption>
{rows}</table>
<blockquote><p>Quoted &quot;text&quot; with &unknown; entity &amp what</p></blockquote>
</article>
'''
PAGE = '''<!DOCTYPE html>
<html lang="et"><head><meta charset="utf-8"><title>{title}</title>
<link rel="stylesheet" href="/app.css">
<style>body {{ font: 16px/1.5 sans-serif; }} p > a {{ color: red; }}</style>
<script>window.data = {{"a": "<p>not a tag</p>", "b": [1, 2, 3]}};
if (a < b && c > d) {{ document.write("</scr" + "ipt>"); }}</script>
</head><body>
<header><nav><ul>{nav}</ul></nav></header>
<main>
{articles}</main>
<footer><form action="/search" method=get><input name=q placeholder="Otsi"><select name=s><option>a<option selected>b</select><textarea>x < y</textarea><button>Go</button></form></footer>
</body></html>
'''


def page(articles):
    nav = ''.join(f'<li><a href="/s/{i}">Section {i}</a>' for i in range(12))
    body = ''.join(ARTICLE.format(i=i, d=i % 28 + 1, rows=''.join(ROW.format(i=i * 10 + r) for r in range(8)))
                   for i in range(articles))
    return PAGE.format(title=f'{articles} articles', nav=nav, articles=body)


MESSAGE = ('<div>Tere <strong>kõigile</strong>! Kas nägite <a href="https://example.com/a?b=1&amp;c=2">'
           'seda</a>? &#x1F600;</div>')
RICH = ('''<div><h1>Release notes</h1><p>Hello <b>team</b>, a few things from this week:</p>
<ul><li>Fixed the <code>&lt;br&gt;</code> handling in <em>pasted</em> text<li>Images now keep their
<a href="https://example.com/docs#images" title="Docs">size</a><li>Faster search &mdash; about 30%</ul>
<blockquote>Quoting <i>someone</i>: "it works on my machine" &copy; 2026</blockquote>
<figure><img src="https://example.com/i/1.png" width="800" height="600" alt="A chart">
<figcaption>Throughput, before &amp; after</figcaption></figure>
<pre>if (a &lt; b) { return c; }</pre><p>Thanks, <span class="mention" data-id="42">@priit</span><br>
<del>old</del> <ins>new</ins><p>Unclosed paragraph and a <b>misnested <i>tag</b> here</i>.
</div>''' * 2)


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument('--rounds', type=int, default=5)
    ap.add_argument('--scale', type=float, default=1.0, help='multiplies every iteration count')
    ap.add_argument('--output', type=Path, default=ROOT / 'bench/results.json')
    args = ap.parse_args()
    build = Path(tempfile.mkdtemp(prefix='htmlparse-bench.'))
    # name: (text, kind, parses per run)
    workloads = {'message': (MESSAGE, 'fragment', 200000), 'rich message': (RICH, 'fragment', 20000),
                 'small': (page(1), 'document', 20000), 'medium': (page(20), 'document', 1000),
                 'large': (page(400), 'document', 50), 'huge': (page(2000), 'document', 8)}
    for i, (text, _, _) in enumerate(workloads.values()):
        (build / f'{i}.html').write_text(text)

    ris = os.environ.get('RIS', 'ris')
    env = dict(os.environ, IYI_PATH=f'{ROOT}:' + subprocess.check_output([ris, 'env', 'IYI_PATH'], text=True).strip(),
               CARGO_TARGET_DIR=str(build / 'target'))
    subprocess.run([ris, 'build', '--release', str(ROOT / 'bench/parse.ri'), '-o', str(build / 'ris')],
                   cwd=ROOT, env=env, check=True)
    subprocess.run(['cargo', 'build', '--release', '--manifest-path', str(ROOT / 'bench/rust/Cargo.toml')],
                   env=env, check=True)
    binaries = {'htmlparse': build / 'ris', 'html5ever': build / 'target/release/htmlparse_bench'}
    versions = {'ris': subprocess.check_output([ris, 'version'], text=True).strip(),
                'rustc': subprocess.check_output(['rustc', '--version'], text=True).strip()}

    rows = [dict(impl=impl, workload=name, kind=kind, bytes=len(text.encode()),
                 iterations=max(1, int(n * args.scale)), binary=str(binary), input=str(build / f'{i}.html'),
                 samples=[], memory=[])
            for impl, binary in binaries.items()
            for i, (name, (text, kind, n)) in enumerate(workloads.items())]

    def run(row, iterations, write):
        report = build / 'time.txt'
        result = subprocess.run(['/usr/bin/time', '-f', '%M', '-o', str(report), row['binary'], row['input'],
                                 str(iterations), row['kind'], '1' if write else '0'],
                                capture_output=True, text=True)
        if result.returncode != 0:
            raise SystemExit(f"{row['impl']} {row['workload']} failed:\n{result.stderr}")
        checksum, parse_ns, write_ns = result.stdout.split()
        return dict(checksum=checksum, parse_ns=int(parse_ns), write_ns=int(write_ns),
                    peak_rss_kib=int(report.read_text().split()[-1]))

    def measure(row):
        sample = run(row, row['iterations'], True)
        if row['bytes'] >= 40000:
            row['memory'].append(run(row, 1, False)['peak_rss_kib'] - run(row, 0, False)['peak_rss_kib'])
        return sample

    for row in rows:
        row['warmup'] = measure(row)
        row['memory'].clear()
    rng = random.Random(42)
    for _ in range(args.rounds):
        order = list(rows)
        rng.shuffle(order)
        for row in order:
            row['samples'].append(measure(row))

    for name in workloads:
        sums = {s['checksum'] for r in rows if r['workload'] == name for s in [r['warmup'], *r['samples']]}
        if len(sums) != 1:
            raise SystemExit(f'{name}: outputs differ: {sums}')
    for row in rows:
        n = row['iterations']
        row['median_parse_us'] = statistics.median(s['parse_ns'] for s in row['samples']) / n / 1000
        row['median_serialize_us'] = statistics.median(s['write_ns'] for s in row['samples']) / n / 1000
        row['median_parse_mib_s'] = row['bytes'] / (row['median_parse_us'] / 1e6) / 2**20
        if row['memory']:
            row['median_parse_memory_kib'] = statistics.median(row['memory'])
    args.output.write_text(json.dumps(dict(platform=platform.platform(), cpu=cpu(), versions=versions,
                                           rounds=args.rounds, workloads={k: v[1] for k, v in workloads.items()},
                                           rows=rows), indent=2) + '\n')

    def size(row):
        return f"{row['bytes']} B" if row['bytes'] < 1024 else f"{row['bytes'] / 1024:,.0f} KiB"

    print('| Input | Parser | Parse, µs | Parse, MiB/s | Serialize, µs |')
    print('| --- | --- | ---: | ---: | ---: |')
    for row in rows_in_order(rows, workloads):
        print(f"| {row['workload']}, {size(row)} | {row['impl']} | {row['median_parse_us']:,.1f} | "
              f"{row['median_parse_mib_s']:.1f} | {row['median_serialize_us']:,.1f} |")
    print()
    print('| Input | Parser | Memory per parse, MiB | Per input byte |')
    print('| --- | --- | ---: | ---: |')
    for row in rows_in_order(rows, workloads):
        if 'median_parse_memory_kib' in row:
            kib = row['median_parse_memory_kib']
            print(f"| {row['workload']}, {size(row)} | {row['impl']} | {kib / 1024:.1f} | "
                  f"{kib * 1024 / row['bytes']:.1f} |")


def rows_in_order(rows, workloads):
    return [r for name in workloads for r in rows if r['workload'] == name]


def cpu():
    try:
        for line in Path('/proc/cpuinfo').read_text().splitlines():
            if line.startswith('model name'):
                return line.split(':', 1)[1].strip()
    except OSError:
        pass
    return platform.processor()


if __name__ == '__main__':
    main()

import assert from 'node:assert/strict'
// The frontend does not depend on a unit-test framework; use Node's built-in runner.
// eslint-disable-next-line test/no-import-node-test
import test from 'node:test'
import { buildBookmarkHtml, parseBookmarkHtml } from '../../src/utils/bookmarks.ts'

// The bookmark file has two directions that have to agree: what the import reads
// and what the export writes. These tests pin the pair — a change on one side
// that the other does not follow shows up here rather than as a mangled tree
// after a round trip through the panel.

const threeLevels = [
  {
    title: 'A',
    items: [{ title: 'A link', url: 'https://a.example/' }],
    children: [
      {
        title: 'A1',
        items: [],
        children: [
          { title: 'A1a', items: [{ title: 'deep', url: 'https://deep.example/?x=1&y=2' }], children: [] },
        ],
      },
    ],
  },
  { title: 'B', items: [{ title: 'B link', url: 'https://b.example/' }], children: [] },
]

test('the format round-trips: what is written parses back unchanged', () => {
  assert.deepEqual(parseBookmarkHtml(buildBookmarkHtml(threeLevels)), threeLevels)
})

test('folders of the same name under different parents stay distinct', () => {
  const html = `<!DOCTYPE NETSCAPE-Bookmark-file-1>
<DL><p>
    <DT><H3>A</H3>
    <DL><p>
        <DT><H3>热点</H3>
        <DL><p>
            <DT><A HREF="https://a.example/">x</A>
        </DL><p>
    </DL><p>
    <DT><H3>B</H3>
    <DL><p>
        <DT><H3>热点</H3>
        <DL><p>
            <DT><A HREF="https://b.example/">y</A>
        </DL><p>
    </DL><p>
</DL><p>`
  const tree = parseBookmarkHtml(html)
  assert.equal(tree.length, 2)
  assert.equal(tree[0].children.length, 1)
  assert.equal(tree[1].children.length, 1)
  assert.equal(tree[0].children[0].title, '热点')
  assert.equal(tree[1].children[0].title, '热点')
  assert.deepEqual(tree[0].children[0].items.map(i => i.url), ['https://a.example/'])
  assert.deepEqual(tree[1].children[0].items.map(i => i.url), ['https://b.example/'])
})

test('the shape Chrome writes is read, including attributes and descriptions', () => {
  const html = `<!DOCTYPE NETSCAPE-Bookmark-file-1>
<META HTTP-EQUIV="Content-Type" CONTENT="text/html; charset=UTF-8">
<TITLE>Bookmarks</TITLE>
<H1>Bookmarks</H1>
<DL><p>
    <DT><H3 ADD_DATE="1700000000" LAST_MODIFIED="1700000000" PERSONAL_TOOLBAR_FOLDER="true">Bookmarks bar</H3>
    <DL><p>
        <DT><A HREF="https://a.example/?x=1&amp;y=2" ADD_DATE="1700000000" ICON="data:image/png;base64,iVBORw0KGgo=">A &amp; B</A>
        <DT><A HREF="https://b.example/" ADD_DATE="1700000000">B</A>
        <DD>a description, which is not a bookmark
    </DL><p>
</DL><p>`
  const tree = parseBookmarkHtml(html)
  assert.equal(tree.length, 1)
  assert.equal(tree[0].title, 'Bookmarks bar')
  assert.deepEqual(tree[0].items, [
    { title: 'A & B', url: 'https://a.example/?x=1&y=2' },
    { title: 'B', url: 'https://b.example/' },
  ])
})

test('an icon attribute holding markup does not split the tag', () => {
  const html = `<DL><p>
    <DT><A HREF="https://c.example/" ICON="data:image/svg+xml,<svg xmlns='http://www.w3.org/2000/svg'><circle r='4'/></svg>">C</A>
</DL><p>`
  const tree = parseBookmarkHtml(html, 'Unfiled')
  assert.deepEqual(tree, [
    { title: 'Unfiled', items: [{ title: 'C', url: 'https://c.example/' }], children: [] },
  ])
})

test('an entry with an empty title or url is skipped', () => {
  const html = `<DL><p>
    <DT><A HREF="https://ok.example/">ok</A>
    <DT><A HREF="https://blank-title.example/"></A>
    <DT><A HREF="">blank url</A>
    <DT><A HREF="javascript:void(0);"></A>
</DL><p>`
  const links = parseBookmarkHtml(html, 'Unfiled')[0].items
  assert.deepEqual(links, [{ title: 'ok', url: 'https://ok.example/' }])
})

test('entities are decoded, including numeric ones', () => {
  const html = `<DL><p>
    <DT><A HREF="https://e.example/?a=1&amp;b=2">x &lt;y&gt; &#39;z&#39; &#x41;</A>
</DL><p>`
  const links = parseBookmarkHtml(html, 'Unfiled')[0].items
  assert.deepEqual(links, [{ title: 'x <y> \'z\' A', url: 'https://e.example/?a=1&b=2' }])
})

test('nesting goes as deep as the file does', () => {
  const html = `<DL><p>
    <DT><H3>L1</H3>
    <DL><p>
        <DT><H3>L2</H3>
        <DL><p>
            <DT><H3>L3</H3>
            <DL><p>
                <DT><H3>L4</H3>
                <DL><p>
                    <DT><A HREF="https://l4.example/">leaf</A>
                </DL><p>
            </DL><p>
        </DL><p>
    </DL><p>
</DL><p>`
  const tree = parseBookmarkHtml(html)
  assert.equal(tree[0].title, 'L1')
  assert.equal(tree[0].children[0].title, 'L2')
  assert.equal(tree[0].children[0].children[0].title, 'L3')
  assert.equal(tree[0].children[0].children[0].children[0].title, 'L4')
  assert.deepEqual(
    tree[0].children[0].children[0].children[0].items.map(i => i.title),
    ['leaf'],
  )
})

test('a folder without its own level does not swallow the entries after it', () => {
  // The previous parser walked every following sibling and collected all of
  // their anchors, so the first folder came back holding the whole file.
  const html = `<DL><p>
    <DT><H3>empty</H3>
    <DT><H3>later</H3>
    <DL><p>
        <DT><A HREF="https://later.example/">only mine</A>
    </DL><p>
</DL><p>`
  const tree = parseBookmarkHtml(html)
  assert.equal(tree.length, 2)
  assert.equal(tree[0].title, 'empty')
  assert.deepEqual(tree[0].items, [])
  assert.deepEqual(tree[0].children, [])
  assert.deepEqual(tree[1].items, [{ title: 'only mine', url: 'https://later.example/' }])
})

test('links listed before any folder land in a folder of their own', () => {
  const html = `<DL><p>
    <DT><A HREF="https://loose.example/">loose</A>
</DL><p>`
  assert.deepEqual(parseBookmarkHtml(html, 'Unfiled'), [
    { title: 'Unfiled', items: [{ title: 'loose', url: 'https://loose.example/' }], children: [] },
  ])
})

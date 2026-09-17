import fs from 'node:fs'

const source = fs.readFileSync('/tmp/51kim.html', 'utf8')
const csl = source.match(/csl:\s*'([\s\S]*?)',\s*\n\s*rui:/)?.[1] ?? ''
const folderNames = {
  '0021': '热门', '003341': '购物', '002120': '影视', '003342': '音乐', '002150': '效率',
  '002130': '邮箱', '002110': '资讯', '002140': '社区', '002160': '学习', '002170': '资源',
  '003149': '综合', '003156': '谷歌', '003157': '谷歌', '003152': '新闻', '003153': '电商', '003150': '影音',
  '003151': '邮箱', '003340': '运动', '003154': '数码', '003339': '娱乐八卦',
}
const records = [...csl.matchAll(/(\d{6})(\d{4})([^0-9]+)(\d{3})(https?:\/\/.*?)(?=052kimzhuye)/g)]
  .map((match) => {
    const prefix = csl.slice(Math.max(0, match.index - 12), match.index + match[0].indexOf(match[3]))
    const code = Object.keys(folderNames).sort((a, b) => b.length - a.length).find((item) => prefix.includes(item))
    return { folder: folderNames[code] ?? '其他', title: match[3], url: match[5] }
  })
const unique = [...new Map(records.map((item) => [item.url, item])).values()]

const escape = (value) => value.replaceAll('&', '&amp;').replaceAll('"', '&quot;').replaceAll('<', '&lt;').replaceAll('>', '&gt;')
const lines = [
  '<!DOCTYPE NETSCAPE-Bookmark-file-1>',
  '<META HTTP-EQUIV="Content-Type" CONTENT="text/html; charset=UTF-8">',
  '<TITLE>导航书签</TITLE>',
  '<H1>导航书签</H1>',
  '<DL><p>',
  ...[...new Map(unique.map((item) => [item.folder, item])).keys()].flatMap((folder) => [
    `    <DT><H3 ADD_DATE="0">${escape(folder)}</H3>`,
    '    <DL><p>',
    ...unique.filter((item) => item.folder === folder).map((item) => `        <DT><A HREF="${escape(item.url)}">${escape(item.title)}</A>`),
    '    </DL><p>',
  ]),
  '</DL><p>',
]

fs.writeFileSync('51kim-bookmarks.html', `${lines.join('\n')}\n`)
console.log(`Exported ${unique.length} unique bookmarks to 51kim-bookmarks.html`)

export interface SearchEngine {
  iconSrc: string
  title: string
  url: string
}

export const searchEngineList: SearchEngine[] = [
  { iconSrc: '/assets/search_engine_svg/bing.svg', title: 'Bing', url: 'https://www.bing.com/search?q=%s' },
  { iconSrc: '/assets/search_engine_svg/google.svg', title: 'Google', url: 'https://www.google.com/search?q=%s' },
  { iconSrc: '/assets/search_engine_svg/baidu.svg', title: 'Baidu', url: 'https://www.baidu.com/s?wd=%s' },
  { iconSrc: '/assets/search_engine_svg/duckduckgo.svg', title: 'DuckDuckGo', url: 'https://duckduckgo.com/?q=%s' },
  { iconSrc: '/assets/search_engine_svg/yahoo.svg', title: 'Yahoo', url: 'https://search.yahoo.com/search?p=%s' },
  { iconSrc: '/assets/search_engine_svg/yandex.png', title: 'Yandex', url: 'https://yandex.com/search/?text=%s' },
  { iconSrc: '/assets/search_engine_svg/ecosia.svg', title: 'Ecosia', url: 'https://www.ecosia.org/search?q=%s' },
  { iconSrc: '/assets/search_engine_svg/brave.svg', title: 'Brave Search', url: 'https://search.brave.com/search?q=%s' },
  { iconSrc: '/assets/search_engine_svg/startpage.svg', title: 'Startpage', url: 'https://www.startpage.com/sp/search?query=%s' },
  { iconSrc: '/assets/search_engine_svg/sogou.svg', title: 'Sogou', url: 'https://www.sogou.com/web?query=%s' },
  { iconSrc: '/assets/search_engine_svg/360.png', title: '360 Search', url: 'https://www.so.com/s?q=%s' },
  { iconSrc: '/assets/search_engine_svg/aol.svg', title: 'AOL', url: 'https://search.aol.com/aol/search?q=%s' },
]

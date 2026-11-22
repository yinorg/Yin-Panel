# Favicon fixture

`POST /api/panel/itemIcon/getSiteFavicon` fetches a page the caller names, so it
cannot be recorded by `app record` (which only replays the app's own HTTP). Run
this directory as a server and compare the two builds by hand:

```sh
(cd backend/record/favicon-fixture && python3 -m http.server 18080 --bind 127.0.0.1 &)

# both against the same fixture
curl -H "Cookie: yin_token=$TOKEN" -H 'Content-Type: application/json' \
  -d '{"url":"http://127.0.0.1:18080/index.html"}' \
  http://127.0.0.1:3020/api/panel/itemIcon/getSiteFavicon   # Rust front door

curl -H "Cookie: yin_token=$TOKEN" -H 'Content-Type: application/json' \
  -d '{"url":"http://127.0.0.1:18080/index.html"}' \
  http://127.0.0.1:<go-child-port>/api/panel/itemIcon/getSiteFavicon
```

- `index.html` declares `shortcut icon` → both answer the PNG, same bytes.
- `plain.html` declares none → both fall back to `/favicon.ico` (the port must
  be kept in the authority).
- A missing page answers `acquisition failed: get ico error:HTTP request failed with status code 404`
  on both.
- The file name is `md5(name + time)` in Go, so the two builds never agree on it;
  compare the extension, the `/uploads/` prefix and the stored bytes.

cargo run -- --help

Remove-Item -Recurse -Force "public"

cargo run -- --inputs "sample_content" `
    --share "X:https://twitter.com/intent/tweet?url={URL}&text={TITLE}%0A%0A{TEXT}" `
    --share "Telegram:https://t.me/share/url?url={URL}&text={TITLE}" `
    --share "LinkedIn:https://www.linkedin.com/sharing/share-offsite/?url={URL}"
    
Set-Location "public"

python -m http.server
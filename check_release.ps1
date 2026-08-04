try {
    $r = Invoke-RestMethod -Uri 'https://api.github.com/repos/BG2MKJ/WinMosh/releases/latest' -Headers @{'Accept'='application/vnd.github+json'}
    Write-Host "tag: $($r.tag_name)"
    foreach ($a in $r.assets) {
        Write-Host "  $($a.name)  $($a.size) bytes"
    }
} catch {
    Write-Host "NO RELEASE YET: $_"
}

<#
.SYNOPSIS
    Screenshot the running RTX Spark AI Control V2 webview, optionally after
    driving the interface to a page.

.DESCRIPTION
    WebView2 exposes a loopback-only CDP endpoint when the application is started
    with WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9222. This
    script talks to it over the loopback WebSocket and asks the compositor for a
    PNG, which avoids needing a screen grab, a Python install or synthetic input
    in the operator's session.

.EXAMPLE
    ./tools/shot.ps1 -Out screenshots/phase3-05-updates.png -Page Updates
#>
param(
    [Parameter(Mandatory = $true)][string]$Out,
    [string]$Page = '',
    [int]$Port = 9222,
    [int]$WaitMs = 900,
    # Device pixel ratio here is 1.5, so 0.667 keeps a 1500px window at its
    # logical size instead of shipping 2250px-wide PNGs in the repository.
    [double]$Scale = 1.0
)

$ErrorActionPreference = 'Stop'

function Receive-Cdp {
    param($Ws, [int]$Id, $Ct)
    $buf = New-Object byte[] 4194304
    $recv = [System.ArraySegment[byte]]::new($buf)
    $stream = [System.IO.MemoryStream]::new()
    do {
        $task = $Ws.ReceiveAsync($recv, $Ct)
        if (-not $task.Wait(30000)) { throw 'timed out waiting for a CDP reply' }
        $stream.Write($buf, 0, $task.Result.Count)
    } while (-not $task.Result.EndOfMessage)
    $text = [System.Text.Encoding]::UTF8.GetString($stream.ToArray())
    $reply = $text | ConvertFrom-Json
    return $reply
}

function Send-Cdp {
    param($Ws, [int]$Id, [string]$Method, $Params, $Ct)
    $payload = @{ id = $Id; method = $Method; params = $Params } |
        ConvertTo-Json -Depth 12 -Compress
    $bytes = [System.Text.Encoding]::UTF8.GetBytes($payload)
    $send = [System.ArraySegment[byte]]::new($bytes)
    $null = $Ws.SendAsync($send, [System.Net.WebSockets.WebSocketMessageType]::Text, $true, $Ct).Wait(8000)
    while ($true) {
        $reply = Receive-Cdp -Ws $Ws -Id $Id -Ct $Ct
        if ($reply.id -eq $Id) { return $reply }
        # event notification: keep reading until the reply for this request lands
    }
}

$list = Invoke-RestMethod -Uri "http://127.0.0.1:$Port/json" -TimeoutSec 5
$target = $list | Where-Object { $_.type -eq 'page' } | Select-Object -First 1
if (-not $target) { throw 'no page target on the debugging port' }

$ws = [System.Net.WebSockets.ClientWebSocket]::new()
$ct = [System.Threading.CancellationToken]::None
try {
    $null = $ws.ConnectAsync([Uri]$target.webSocketDebuggerUrl, $ct).Wait(6000)

    if ($Page) {
        $js = @"
(() => {
  const item = [...document.querySelectorAll('.nav-item')]
    .find((b) => b.textContent.trim().toLowerCase() === '$($Page.ToLower())');
  if (!item) return 'no such page';
  item.click();
  return 'ok';
})()
"@
        $reply = Send-Cdp -Ws $ws -Id 1 -Method 'Runtime.evaluate' -Ct $ct -Params @{
            expression    = $js
            returnByValue = $true
            awaitPromise  = $true
        }
        $clicked = $reply.result.result.value
        if ($clicked -ne 'ok') { throw "could not open page '$Page': $clicked" }
        Start-Sleep -Milliseconds $WaitMs
    }

    $metrics = Send-Cdp -Ws $ws -Id 3 -Method 'Page.getLayoutMetrics' -Ct $ct -Params @{}
    $viewport = $metrics.result.cssLayoutViewport
    $shot = Send-Cdp -Ws $ws -Id 2 -Method 'Page.captureScreenshot' -Ct $ct -Params @{
        format                = 'png'
        captureBeyondViewport = $false
        fromSurface           = $true
        clip                  = @{
            x      = 0
            y      = 0
            width  = $viewport.clientWidth
            height = $viewport.clientHeight
            scale  = $Scale
        }
    }
    if (-not $shot.result.data) { throw 'no screenshot data returned' }

    $dir = Split-Path -Parent $Out
    if ($dir -and -not (Test-Path $dir)) { $null = New-Item -ItemType Directory -Path $dir }
    [System.IO.File]::WriteAllBytes(
        [System.IO.Path]::GetFullPath($Out),
        [System.Convert]::FromBase64String($shot.result.data)
    )
    $size = (Get-Item $Out).Length
    Write-Output "wrote $Out ($size bytes)"
}
finally {
    if ($ws.State -eq [System.Net.WebSockets.WebSocketState]::Open) {
        $null = $ws.CloseAsync([System.Net.WebSockets.WebSocketCloseStatus]::NormalClosure, 'done', $ct).Wait(2000)
    }
    $ws.Dispose()
}

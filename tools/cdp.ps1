<#
.SYNOPSIS
    Evaluate JavaScript inside the running RTX Spark AI Control V2 webview.

.DESCRIPTION
    Drives the UI for screenshots without synthesising input into the user's
    session: the app is started with WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=
    --remote-debugging-port=9222, and this script talks CDP over the loopback
    WebSocket that WebView2 exposes. Loopback only; the port is not listening on
    any external interface and the app is never shipped with it enabled.

.EXAMPLE
    ./tools/cdp.ps1 -Expression "document.title"
#>
param(
    [Parameter(Mandatory = $true)][string]$Expression,
    [int]$Port = 9222
)

$ErrorActionPreference = 'Stop'

$targets = Invoke-RestMethod -Uri "http://127.0.0.1:$Port/json" -TimeoutSec 5
$page = $targets | Where-Object { $_.type -eq 'page' } | Select-Object -First 1
if (-not $page) { throw 'no page target on the debugging port' }

$ws = [System.Net.WebSockets.ClientWebSocket]::new()
$ct = [System.Threading.CancellationToken]::None
try {
    $null = $ws.ConnectAsync([Uri]$page.webSocketDebuggerUrl, $ct).Wait(5000)

    $msg = @{
        id     = 1
        method = 'Runtime.evaluate'
        params = @{
            expression    = $Expression
            returnByValue = $true
            awaitPromise  = $true
        }
    } | ConvertTo-Json -Depth 8 -Compress

    $bytes = [System.Text.Encoding]::UTF8.GetBytes($msg)
    $send = [System.ArraySegment[byte]]::new($bytes)
    $null = $ws.SendAsync($send, [System.Net.WebSockets.WebSocketMessageType]::Text, $true, $ct).Wait(5000)

    $buf = New-Object byte[] 262144
    $recv = [System.ArraySegment[byte]]::new($buf)
    $stream = [System.IO.MemoryStream]::new()
    do {
        $task = $ws.ReceiveAsync($recv, $ct)
        if (-not $task.Wait(10000)) { throw 'timed out waiting for CDP reply' }
        $stream.Write($buf, 0, $task.Result.Count)
    } while (-not $task.Result.EndOfMessage)

    $text = [System.Text.Encoding]::UTF8.GetString($stream.ToArray())
    $reply = $text | ConvertFrom-Json
    if ($reply.result.exceptionDetails) {
        Write-Output ("EXCEPTION: " + $reply.result.exceptionDetails.text)
        exit 2
    }
    $value = $reply.result.result.value
    if ($null -eq $value) { Write-Output 'null' } else { Write-Output $value }
}
finally {
    if ($ws.State -eq [System.Net.WebSockets.WebSocketState]::Open) {
        $null = $ws.CloseAsync([System.Net.WebSockets.WebSocketCloseStatus]::NormalClosure, 'done', $ct).Wait(2000)
    }
    $ws.Dispose()
}

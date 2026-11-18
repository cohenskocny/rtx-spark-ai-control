<#
.SYNOPSIS
    Downscale screenshots to the size they are read at, without touching the
    originals.

.DESCRIPTION
    The window paints at a device pixel ratio of 1.5, so a 1500x950 window is
    captured as 2250x1425. That is right for evidence and wrong for a
    repository: twenty such PNGs came to 65 MB.

    Scaled PNG is still large for this interface — large flat gradients and
    antialiased text compress badly — so the public set is written as JPEG at a
    quality where the text stays legible. Originals are never overwritten; this
    script only reads -Src and writes -Out.

.EXAMPLE
    ./tools/optimize-shots.ps1 -Src C:\archive\screenshots-original -Out screenshots
    ./tools/optimize-shots.ps1 -Src C:\archive\screenshots-original -Out shots -Format png
#>
param(
    [Parameter(Mandatory = $true)][string]$Src,
    [Parameter(Mandatory = $true)][string]$Out,
    [int]$Width = 1400,
    [ValidateSet('jpeg', 'png')][string]$Format = 'jpeg',
    [int]$Quality = 92
)

$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing

if (-not (Test-Path $Out)) { New-Item -ItemType Directory -Path $Out | Out-Null }

$codec = [System.Drawing.Imaging.ImageCodecInfo]::GetImageEncoders() |
    Where-Object { $_.MimeType -eq 'image/jpeg' }
$extension = if ($Format -eq 'jpeg') { '.jpg' } else { '.png' }

$rows = @()
foreach ($file in Get-ChildItem -Path $Src -Filter *.png) {
    $image = [System.Drawing.Image]::FromFile($file.FullName)
    try {
        $scale = [Math]::Min(1.0, $Width / $image.Width)
        $w = [int][Math]::Round($image.Width * $scale)
        $h = [int][Math]::Round($image.Height * $scale)

        $bitmap = New-Object System.Drawing.Bitmap($w, $h)
        $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
        try {
            $graphics.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
            $graphics.PixelOffsetMode = [System.Drawing.Drawing2D.PixelOffsetMode]::HighQuality
            $graphics.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::HighQuality
            $graphics.DrawImage($image, 0, 0, $w, $h)
        } finally {
            $graphics.Dispose()
        }

        $target = Join-Path $Out ([System.IO.Path]::GetFileNameWithoutExtension($file.Name) + $extension)
        if ($Format -eq 'jpeg') {
            $parameters = New-Object System.Drawing.Imaging.EncoderParameters(1)
            $parameters.Param[0] = New-Object System.Drawing.Imaging.EncoderParameter(
                [System.Drawing.Imaging.Encoder]::Quality, [int]$Quality)
            $bitmap.Save($target, $codec, $parameters)
        } else {
            $bitmap.Save($target, [System.Drawing.Imaging.ImageFormat]::Png)
        }
        $bitmap.Dispose()

        $rows += [pscustomobject]@{
            Name = Split-Path $target -Leaf
            From = "$($image.Width)x$($image.Height) $([Math]::Round($file.Length / 1KB)) kB"
            To   = "$($w)x$($h) $([Math]::Round((Get-Item $target).Length / 1KB)) kB"
        }
    } finally {
        $image.Dispose()
    }
}

$rows | Format-Table -AutoSize
$total = (Get-ChildItem -Path $Out -File | Measure-Object Length -Sum).Sum
"Total in ${Out}: $([Math]::Round($total / 1MB, 1)) MB"

# The published Windows app run as a person runs it: the release before the latest installed, the update
# taken through the app's own keys, the install ping answered, then a feedback report when asked.
# live.ps1 [-From <version>] [-Report]; pictures, the accessibility tree and the app's files land in .\live.
param([string]$From = '', [switch]$Report)
$ErrorActionPreference = 'Stop'
$releases = '5e1y/open-negative-initiative-releases'
$out = Join-Path $PWD 'live'
$work = Join-Path $env:RUNNER_TEMP 'oni-live'
New-Item -ItemType Directory -Force $out, $work | Out-Null

Add-Type -AssemblyName System.Windows.Forms, System.Drawing, UIAutomationClient, UIAutomationTypes
$AE = [System.Windows.Automation.AutomationElement]
$Scope = [System.Windows.Automation.TreeScope]

function Shot([string]$name) {
    $b = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
    $bitmap = New-Object System.Drawing.Bitmap $b.Width, $b.Height
    [System.Drawing.Graphics]::FromImage($bitmap).CopyFromScreen($b.Location, [System.Drawing.Point]::Empty, $b.Size)
    $bitmap.Save((Join-Path $out "$name.png"))
}

# Every element the app's windows expose, as the accessibility tree names them.
function Elements([int]$id) {
    $mine = New-Object System.Windows.Automation.PropertyCondition($AE::ProcessIdProperty, $id)
    foreach ($w in $AE::RootElement.FindAll($Scope::Children, $mine)) {
        $w
        foreach ($e in $w.FindAll($Scope::Descendants, [System.Windows.Automation.Condition]::TrueCondition)) { $e }
    }
}

function Dump([int]$id, [string]$name) {
    Elements $id | ForEach-Object { "{0} | {1} | enabled={2}" -f $_.Current.ControlType.ProgrammaticName, $_.Current.Name, $_.Current.IsEnabled } |
        Set-Content -Encoding utf8 (Join-Path $out "$name.txt")
}

# The first enabled button whose name passes `$match`, waited for; the screen and the tree kept when none comes.
function Await-Button([int]$id, [scriptblock]$match, [int]$seconds, [string]$what) {
    for ($t = 0; $t -lt $seconds; $t += 2) {
        $hit = Elements $id | Where-Object { $_.Current.ControlType -eq [System.Windows.Automation.ControlType]::Button -and $_.Current.IsEnabled -and (& $match $_.Current.Name) } | Select-Object -First 1
        if ($hit) { return $hit }
        Start-Sleep 2
    }
    Shot "missing-$what"
    Dump $id "missing-$what"
    throw "no $what after $seconds s"
}

function Press($element) {
    $element.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke()
}

function Running([string]$exe) {
    Get-Process -Name ONI -ErrorAction SilentlyContinue | Where-Object { $_.Path -eq $exe } | Select-Object -First 1
}

function Remembered([string]$support, [string]$key) {
    $file = Join-Path $support 'preferences.json'
    if (Test-Path $file) { (Get-Content -Raw $file | ConvertFrom-Json).$key }
}

function Keep-Files([string]$support) {
    Copy-Item (Join-Path $support 'preferences.json') $out -ErrorAction SilentlyContinue
    Copy-Item (Join-Path $support 'Logs\oni.log') $out -ErrorAction SilentlyContinue
}

# The latest release, and the one the app starts from.
$tags = (Invoke-RestMethod "https://api.github.com/repos/$releases/releases?per_page=5") | ForEach-Object { $_.tag_name.TrimStart('v') }
$latest = $tags[0]
if (-not $From) { $From = $tags[1] }
Write-Host "from $From to $latest"

function Unpacked([string]$version, [string]$into) {
    $zip = Join-Path $work "ONI-$version.zip"
    Invoke-WebRequest -UseBasicParsing "https://github.com/$releases/releases/download/v$version/ONI-windows.zip" -OutFile $zip
    Expand-Archive -Path $zip -DestinationPath $into -Force
    (Get-ChildItem $into -Recurse -Filter ONI.exe | Select-Object -First 1).FullName
}
$exe = Unpacked $From (Join-Path $work 'app')
$expected = (Get-FileHash (Unpacked $latest (Join-Path $work 'latest'))).Hash
$before = (Get-FileHash $exe).Hash

# A support folder of the check's own: the version marked launched, so only the update's own ping is
# sent, and the release's welcome marked seen. Without a BOM, which the app's JSON reader refuses.
$support = Join-Path $work 'support'
New-Item -ItemType Directory -Force $support | Out-Null
$edition = ($latest -split '\.')[0..1] -join '.'
[System.IO.File]::WriteAllText((Join-Path $support 'preferences.json'), (@{ lastLaunchedVersion = $From; welcomeSeen = $edition } | ConvertTo-Json))
$env:ONI_SUPPORT = $support

$app = Start-Process $exe -PassThru
try {
    $update = Await-Button $app.Id { param($n) $n -eq "UPDATE" -or $n -like "UPDATE TO $latest" -or $n -like "Download version $latest*" } 180 'update key'
    Shot '1-update-key'
    Press $update
    Press (Await-Button $app.Id { param($n) $n -eq 'Download' } 60 'download key')
    if (-not $app.WaitForExit(300000)) {
        Shot 'missing-quit'
        Dump $app.Id 'missing-quit'
        throw 'the app did not quit once its update was downloaded'
    }
    # The helper swaps ONI.exe and starts it; the old copy goes once the new one runs.
    for ($t = 0; $t -lt 120 -and -not ((Get-FileHash $exe).Hash -eq $expected -and (Running $exe)); $t += 2) { Start-Sleep 2 }
    $hash = (Get-FileHash $exe).Hash
    if ($hash -ne $expected) { throw "ONI.exe is not $latest's: $(if ($hash -eq $before) { "$From's is back" } else { 'another file' })" }
    $updated = Running $exe
    if (-not $updated) { throw "$latest's ONI.exe does not run" }
    for ($t = 0; $t -lt 60 -and (Remembered $support 'lastLaunchedVersion') -ne $latest; $t += 2) { Start-Sleep 2 }
    $marked = Remembered $support 'lastLaunchedVersion'
    if ($marked -ne $latest) { throw "the install ping was not answered: lastLaunchedVersion is $marked" }
    Start-Sleep 10
    Shot '2-updated'
    if (Test-Path "$exe.previous") { throw "the former ONI.exe was left beside the new one" }
    Write-Host "updated from $From to $latest, and the update was reported as one"

    if ($Report) {
        Press (Await-Button $updated.Id { param($n) $n -eq 'Feedback' -or $n -eq 'Report a Bug / Give Feedback' } 60 'feedback key')
        Await-Button $updated.Id { param($n) $n -eq 'Cancel' } 30 'report sheet' | Out-Null
        $message = "Check from the Windows CI: $From updated to $latest through the app, then this report. Sent on purpose, to delete."
        $field = Elements $updated.Id | Where-Object { $_.Current.ControlType -eq [System.Windows.Automation.ControlType]::Edit } | Select-Object -First 1
        try { $field.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern).SetValue($message) } catch { Write-Host "the field takes no value: $_" }
        $send = $null
        try { $send = Await-Button $updated.Id { param($n) $n -eq 'Send' } 6 'send key by value' } catch {}
        if (-not $send) {
            # The field holds the focus as the sheet opens; the keys reach it once the window is in front.
            $shell = New-Object -ComObject WScript.Shell
            $shell.AppActivate($updated.Id) | Out-Null
            Start-Sleep 1
            $shell.SendKeys(($message -replace '([+^%~(){}\[\]])', '{$1}'))
            $send = Await-Button $updated.Id { param($n) $n -eq 'Send' } 30 'send key'
        }
        Press $send
        Await-Button $updated.Id { param($n) $n -eq 'Done' } 90 'report sent' | Out-Null
        Shot '3-report-sent'
        Write-Host 'the report was accepted'
    }
} finally {
    Dump $(if ($updated) { $updated.Id } else { $app.Id }) 'tree'
    Get-Process -Name ONI -ErrorAction SilentlyContinue | Stop-Process -Force
    Keep-Files $support
}

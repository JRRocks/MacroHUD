<#
Example external macro for MacroHUD (docs/MACROS-EXTERNES.md). It pretends to click every two
seconds until its click limit, and speaks MacroHUD's protocol on its standard input and output. It
has nothing to do with any particular program, and touches neither the mouse nor the keyboard unless
asked to (-Clicks, for the tests on the PC, Windows only).

Runs on Windows PowerShell 5.1 and on PowerShell 7 (Windows, Linux); saved in UTF-8 with a BOM, so
that Windows PowerShell 5.1 reads its accents right. MacroHUD's tests (essais.rs, next to it) start
it in each of its modes; LISEZMOI.md says how to declare it on the PC.
#>
param(
    # normal: counts its clicks up to its limit, then says « fin ».
    # wait: first waits (« attente »), then as normal.
    # error: quits in error after its first click (exit code 3), without « fin ».
    # alert: sends an alert, then quits (exit code 1) without « fin ».
    # deaf: hears neither « arret » nor the end of its input: MacroHUD must stop it by force.
    [ValidateSet('normal', 'wait', 'error', 'alert', 'deaf')]
    [string] $Mode = 'normal',
    # Time between two clicks, in milliseconds.
    [ValidateRange(10, 60000)]
    [int] $Interval = 2000,
    # none: only counts. click: a real click of -Button each time. hold: keeps -Button down from its
    # first click to its end (tests on the PC only, Windows).
    [ValidateSet('none', 'click', 'hold')]
    [string] $Clicks = 'none',
    [ValidateSet('left', 'right', 'middle')]
    [string] $Button = 'right',
    # Does not heed « pause »: to check that MacroHUD drops the injected clicks on its HUD (D-22).
    [switch] $Stubborn,
    # The exit code of the modes error and alert (-1: 3 and 1).
    [int] $ExitCode = -1
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version 2.0

# UTF-8 both ways, whatever the code page of the console; each line goes out at once. Nothing else
# may write to the standard output (MacroHUD would read it).
$utf8 = New-Object System.Text.UTF8Encoding $false
$script:toHud = [System.IO.StreamWriter]::new([Console]::OpenStandardOutput(), $utf8)
$script:toHud.AutoFlush = $true
$script:fromHud = [System.IO.StreamReader]::new([Console]::OpenStandardInput(), $utf8)
$script:pending = $script:fromHud.ReadLineAsync()
$script:listening = $true

$script:limit = 100
$script:count = 0
$script:paused = $false
$script:held = $false

if ($Clicks -ne 'none') {
    Add-Type -Namespace MacroHudExample -Name Mouse -MemberDefinition @'
[DllImport("user32.dll")]
public static extern void mouse_event(uint flags, uint dx, uint dy, uint data, System.UIntPtr extra);
'@
}
# MOUSEEVENTF_*DOWN, MOUSEEVENTF_*UP.
$script:flags = @{ left = @(0x0002, 0x0004); right = @(0x0008, 0x0010); middle = @(0x0020, 0x0040) }[$Button]

function Say([string] $line) {
    $script:toHud.WriteLine($line)
}

function PressButton {
    [MacroHudExample.Mouse]::mouse_event([uint32] $script:flags[0], 0, 0, 0, [UIntPtr]::Zero)
}

function ReleaseButton {
    [MacroHudExample.Mouse]::mouse_event([uint32] $script:flags[1], 0, 0, 0, [UIntPtr]::Zero)
}

function ClickOnce {
    if ($Clicks -eq 'click') {
        PressButton
        Start-Sleep -Milliseconds 30
        ReleaseButton
    } elseif ($Clicks -eq 'hold' -and -not $script:held) {
        PressButton
        $script:held = $true
    }
}

# Quits, after releasing what it holds.
function Finish([int] $code) {
    if ($script:held) {
        ReleaseButton
        $script:held = $false
    }
    $script:toHud.Flush()
    exit $code
}

# One line from MacroHUD.
function Hear([string] $line) {
    $words = @($line.Trim() -split '\s+', 3)
    switch ($words[0]) {
        'reglage' {
            $value = 0
            if ($words.Count -eq 3 -and $words[1] -eq 'limite' -and [int]::TryParse($words[2], [ref] $value)) {
                $script:limit = $value
                Say "etat $($script:count)/$($script:limit)"
            }
        }
        'pause' {
            if (-not $Stubborn) {
                $script:paused = $true
                if ($script:held) {
                    ReleaseButton
                    $script:held = $false
                }
                Say 'pastille ambre Pause'
            }
        }
        'reprise' {
            if ($script:paused) {
                $script:paused = $false
                Say 'pastille verte Clics'
            }
        }
        'arret' {
            if ($Mode -ne 'deaf') {
                Say 'journal arrêt demandé'
                Finish 0
            }
        }
    }
}

# Listens to MacroHUD until `until` (UTC), handling each line as it comes.
function Listen([datetime] $until) {
    while ($true) {
        $left = [int][Math]::Max(0, ($until - [DateTime]::UtcNow).TotalMilliseconds)
        if (-not $script:listening) {
            Start-Sleep -Milliseconds $left
            return
        }
        if (-not $script:pending.Wait($left)) {
            return
        }
        $line = $script:pending.Result
        if ($null -eq $line) {
            # The end of its input: MacroHUD is gone. The same as « arret ».
            $script:listening = $false
            if ($Mode -ne 'deaf') {
                Finish 0
            }
            continue
        }
        $script:pending = $script:fromHud.ReadLineAsync()
        Hear $line
    }
}

Say "journal mode $Mode, un clic toutes les $Interval ms"
if ($Mode -eq 'wait') {
    Say 'pastille grise Attente'
    Say 'attente attend sa fenêtre'
    Listen ([DateTime]::UtcNow.AddMilliseconds(3 * $Interval))
    Say 'reprise'
}
Say 'pastille bleue Départ'
Say "etat $($script:count)/$($script:limit)"
if ($Mode -eq 'alert') {
    Listen ([DateTime]::UtcNow.AddMilliseconds($Interval))
    Say "alerte La fenêtre visée est en plein écran. | Passe-la en fenêtre : en plein écran, la macro ne voit pas l'écran."
    Finish $(if ($ExitCode -ge 0) { $ExitCode } else { 1 })
}

$next = [DateTime]::UtcNow.AddMilliseconds($Interval)
while ($true) {
    Listen $next
    $next = $next.AddMilliseconds($Interval)
    if ($next -lt [DateTime]::UtcNow) {
        # Late (the computer was busy): the beat starts again rather than clicking in a burst.
        $next = [DateTime]::UtcNow.AddMilliseconds($Interval)
    }
    if ($script:paused) {
        continue
    }
    if ($script:count -lt $script:limit) {
        $script:count++
        ClickOnce
        if ($script:count -eq 1) {
            Say 'pastille verte Clics'
        }
        Say "etat $($script:count)/$($script:limit)"
        if ($Mode -eq 'error') {
            [Console]::Error.WriteLine('Exemple : erreur voulue (mode error).')
            Finish $(if ($ExitCode -ge 0) { $ExitCode } else { 3 })
        }
    }
    if ($script:count -ge $script:limit -and $Mode -ne 'deaf') {
        Say 'pastille'
        if ($script:count -eq $script:limit) {
            Say "fin $($script:count)/$($script:limit) clics"
        } else {
            Say "fin $($script:count) clics"
        }
        Finish 0
    }
}

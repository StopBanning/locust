param(
  [string]$Tasks = 'bug,feature,refactor,tests,html',
  [ValidateRange(1,20)][int]$Runs = 3,
  [ValidateRange(0.000001,100000)][double]$MaxDollars = 20,
  [ValidateSet('free','api')][string]$Judges = 'free'
)

# The key never enters argv, shell history or a file. The child removes it from its environment before spawning Locust.
Add-Type -AssemblyName PresentationFramework
$window = New-Object System.Windows.Window
$window.Title = 'Locust API benchmark'
$window.Width = 480
$window.Height = 250
$window.WindowStartupLocation = 'CenterScreen'
$window.ResizeMode = 'NoResize'
$panel = New-Object System.Windows.Controls.StackPanel
$panel.Margin = 20
$notice = New-Object System.Windows.Controls.TextBlock
$notice.TextWrapping = 'Wrap'
$notice.Text = "This spends Anthropic API credits. Estimated-spend stop: $MaxDollars dollars; a group already started can finish above it. Paste your API key. It is masked, used in a throwaway Locust profile and never recorded."
$panel.Children.Add($notice) | Out-Null
$keyBox = New-Object System.Windows.Controls.PasswordBox
$keyBox.Margin = '0,16,0,16'
$panel.Children.Add($keyBox) | Out-Null
$button = New-Object System.Windows.Controls.Button
$button.Content = 'Run benchmark on API credits'
$button.Add_Click({ if ($keyBox.Password.Length -gt 0) { $window.DialogResult = $true } })
$panel.Children.Add($button) | Out-Null
$window.Content = $panel
if ($window.ShowDialog() -ne $true) { $keyBox.Clear(); exit 0 }

$previousSpend = $env:LOCUST_SPEND
$previousKey = $env:LOCUST_OWN_MODEL_KEY
try {
  $env:LOCUST_OWN_MODEL_KEY = $keyBox.Password
  $keyBox.Clear()
  $env:LOCUST_SPEND = '1'
  Push-Location (Split-Path -Parent $PSScriptRoot)
  try {
    & node _tools/drive-api-benchmark.mjs --tasks $Tasks --runs $Runs --max-dollars $MaxDollars.ToString([System.Globalization.CultureInfo]::InvariantCulture) --judges $Judges
    $benchmarkExit = $LASTEXITCODE
  } finally { Pop-Location }
} finally {
  $keyBox.Clear()
  $env:LOCUST_OWN_MODEL_KEY = $previousKey
  $env:LOCUST_SPEND = $previousSpend
}
exit $benchmarkExit


$metadataJson = cargo metadata --format-version 1 --no-deps
$metadata = $metadataJson | ConvertFrom-Json
$targetDir = $metadata.target_directory

New-Item -ItemType Directory "./finalOutput" -Force
$outputDir = Resolve-Path "./finalOutput"

echo "Target directory at $targetDir"
echo "Putting final output into $outputDir"

$targets = @(
    "x86_64-pc-windows-gnu"
    "x86_64-unknown-linux-gnu"
)

Remove-Item -Path "$outputDir/*" -Recurse -Force

Set-Location ./moonlight-web

echo "------------- Starting Build for Frontend -------------"
New-Item -ItemType Directory "$outputDir/static" -Force | Out-Null

Remove-Item -Path "./dist" -Recurse -Force
npm run build

Copy-Item -Path "./dist/*" -Destination "$outputDir/static" -Recurse -Force
echo "------------- Finished Build for Frontend -------------"

foreach($target in $targets) {
    echo "------------- Starting Build for $target -------------"
    $messages = cross build --release --target $target --message-format=json | ForEach-Object { $_ | ConvertFrom-Json }
    if ($LASTEXITCODE -ne 0) {
        exit $LASTEXITCODE
    }
    echo "------------- Finished Build for $target -------------"

    $artifact = $messages | Where-Object { $_.reason -eq "compiler-artifact" -and $_.executable }
    $binaryPaths = $artifact | ForEach-Object { Join-Path -Path $targetDir -ChildPath ($_.executable.Substring("/target".length)) }

    $binaryPaths | ForEach-Object { Write-Host "Binary: $_" }

    echo "------------- Starting Zipping for $target -------------"
    $itemsToZip = @($binaryPaths) + "$outputDir/static"
    $zipDestination = "../finalOutput/moonlight-web-$target.zip"

    Compress-Archive -Path $itemsToZip -DestinationPath $zipDestination -Force

    echo "Created Zip file at $zipDestination"
    echo "------------- Finished Zipping for $target -------------"
}
Remove-Item -Path "$outputDir" -Recurse -Force

cd ..

echo "Finished!"
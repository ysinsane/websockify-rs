$serviceName = "Websockify"
$webFolder = "C:\projs\rust\websockify-rs\assets"
$websockifyExe = "C:\projs\rust\ws-rs\target\release\websockify-rs.exe"
$vncAddress = "127.0.0.1:5900"
$webHost = "0.0.0.0:80"

if (Get-Service $serviceName -ErrorAction SilentlyContinue)
{
    $serviceToRemove = Get-WmiObject -Class Win32_Service -Filter "name='$serviceName'"
    $serviceToRemove.delete()
    "service removed"
}
else
{
    "service does not exists"
}

"installing service"


# $secpasswd = ConvertTo-SecureString "SomePassword" -AsPlainText -Force
# $mycreds = New-Object System.Management.Automation.PSCredential (".\SomeUser", $secpasswd)
$binaryPath = "$websockifyExe --web $webFolder --source $webHost --target $vncAddress"

# 安装服务
New-Service -name $serviceName -binaryPathName $binaryPath -displayName $serviceName -startupType Automatic -credential $mycreds

# 开启服务
Start-Service -Name $serviceName
"installation completed"
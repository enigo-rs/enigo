# Permissions

## Linux
No elevated privileges are needed

## Windows
[UIPI](https://en.wikipedia.org/wiki/User_Interface_Privilege_Isolation) is a security measure that "prevents processes with a lower "integrity level" (IL) from sending messages to higher IL processes". If your program does not have elevated privileges, you won't be able to use `enigo` is some situations. It won't be possible to use it with the task manager for example. Run your program as an admin, if you need to use `enigo` with processes with a higher "integrity level".

## macOS
The user needs to grant the application the permission to access their Mac. Official instructions on how to do that can be found [here](https://web.archive.org/web/20231005204542/https://support.apple.com/guide/mac-help/allow-accessibility-apps-to-access-your-mac-mh43185/mac). Enigo will check if the application has the needed permissions and ask the user to grant them if the permissions are missing. You can change this behavior with the settings when creating the Enigo struct.
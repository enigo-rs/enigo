# Permissions

## Linux
No elevated privileges are needed

## Windows
[UIPI](https://en.wikipedia.org/wiki/User_Interface_Privilege_Isolation) is a security measure that "prevents processes with a lower "integrity level" (IL) from sending messages to higher IL processes". If your program does not have elevated privileges, you won't be able to use `enigo` is some situations. It won't be possible to use it with the task manager for example. Run your program as an admin, if you need to use `enigo` with processes with a higher "integrity level".

## macOS
You need to grant the application to access your Mac. You can find official instructions [here](https://web.archive.org/web/20231005204542/https://support.apple.com/guide/mac-help/allow-accessibility-apps-to-access-your-mac-mh43185/mac).
# Reproduce from the repository root: bash docs/demo/record.sh
# Never run directly in a real HOME. record.sh provides an empty environment.
Require prj
Require git
Output docs/demo/prjump.gif
Output docs/demo/prjump.mp4
Set Shell "bash"
Set Width 1200
Set Height 410
Set FontSize 20
Set FontFamily "Menlo"
Set Theme "Github"
Set Padding 24
Set Framerate 15
Set TypingSpeed 30ms
Set CursorBlink false

Hide
Type `export PS1='$ '; cd "$PRJ_DEMO_ROOT/repo"; clear`
Enter
Sleep 300ms
Show
Type@1ms "# GitHub: your branch, ready for a pull request"
Enter
Type "git switch -qc feature/cart"
Enter
Type "git commit --allow-empty -qm 'Add cart'"
Enter
Sleep 2s
Hide
Type `git push -qu origin feature/cart && git remote set-url origin git@github.com:octo-demo/shop-api.git; clear`
Enter
Sleep 200ms
Show
Type@1ms "# GitHub: open the new pull request page (URL preview)"
Enter
Type "prj open --print"
Enter
Sleep 4s

Ctrl+L
Type@1ms "# Bitbucket Cloud: same command, another remote"
Enter
Type "prj open --remote bitbucket --print"
Enter
Sleep 4s

Ctrl+L
Type@1ms "# Choose the target branch"
Enter
Type "prj open -t develop --print"
Enter
Sleep 4s

Ctrl+L
Type@1ms "# SSH host alias: demo-gh maps to github.com"
Enter
Type `cat "$PRJ_CONFIG"`
Enter
Sleep 1s
Type "prj open --remote alias --print"
Enter
Sleep 5s

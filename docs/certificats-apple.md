# Certificats Apple (iPhone : montage de l'image développeur)

## Symptôme

Le tunnel iOS est actif, mais l'image développeur ne se monte pas. L'écran affiche :

> Windows ne fait pas confiance au certificat d'Apple : l'image développeur ne peut pas être signée.

## Cause

ShareScreen demande à Apple (`gs.apple.com`) une signature pour l'image développeur. Si le magasin de certificats de Windows ne contient pas la chaîne Apple, la connexion est refusée. Le serveur d'Apple envoie bien son intermédiaire ; le problème vient du PC.

## Correctif (une seule fois, en administrateur)

Il faut deux fichiers officiels d'Apple :

1. **Apple Root CA** : `https://www.apple.com/appleca/AppleIncRootCertificate.cer`
   - Empreinte SHA-1 attendue : `61:1E:5B:66:2C:59:3A:08:FF:58:D1:4A:E2:24:52:D1:98:DF:6C:60`
2. **Apple Server Authentication CA** : l'intermédiaire envoyé par `gs.apple.com`.

Le second fichier s'extrait de la connexion à Apple, avec Node.js :

```bash
node -e "const tls=require('tls');const fs=require('fs');const s=tls.connect({host:'gs.apple.com',port:443,servername:'gs.apple.com',rejectUnauthorized:false},()=>{fs.writeFileSync('AppleServerAuthenticationCA.cer',s.getPeerCertificate(true).issuerCertificate.raw);s.end();});"
```

Vérifiez l'empreinte de la racine, puis, dans PowerShell **en administrateur**, dans le dossier contenant les deux fichiers :

```powershell
certutil -addstore -f Root AppleIncRootCertificate.cer
certutil -addstore -f CA AppleServerAuthenticationCA.cer
```

Relancez ensuite ShareScreen et cliquez sur « Réessayer ».

## Sécurité

Ces certificats font confiance à Apple pour les connexions de ce PC. Ne les installez que depuis les adresses ci-dessus. Pour les retirer, supprimez « Apple Root CA » et « Apple Server Authentication CA » dans `certmgr.msc`.

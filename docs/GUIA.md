# Inq Studio — manual

Inq Studio es un visor y editor de PDF para Windows. Este manual recorre lo que hace, en el orden en que la
mayoría lo necesita. El mismo texto se abre dentro de la aplicación con `F1`. *An English version is in
[GUIDE.md](GUIDE.md).*

## Instalar y actualizar

Descargue el instalador desde la [página de versiones](https://github.com/Voltiq/Inq-Studio/releases/latest)
y ejecútelo. Se instala para el usuario actual y no necesita derechos de administrador. El instalador aún no
está firmado: si SmartScreen avisa, elija *Más información › Ejecutar de todas formas*.

Después de instalar, un PDF se abre en Inq Studio con *Abrir con* en el Explorador; allí puede convertirlo en
la aplicación predeterminada.

Inq Studio busca una versión nueva cada vez que se inicia. Cuando hay una, pregunta si desea instalarla:
**Actualizar ahora**, **Más tarde** (vuelve a preguntar en el siguiente inicio) u **Omitir esta versión**
(solo vuelve a preguntar con la versión posterior). Marque *No preguntar al iniciar* para recibir un aviso
breve en lugar de la pregunta; el mismo interruptor está en Ajustes. *Ajustes › Actualizaciones* comprueba
en el momento e instala. Las actualizaciones están firmadas, y la firma se comprueba antes de instalar nada.

## Primeros pasos

1. Abra un PDF: el botón de la carpeta, `Ctrl+O`, o suelte archivos en cualquier lugar de la ventana.
2. Lea, marque o edite con las herramientas situadas sobre la página.
3. Guarde con `Ctrl+S`, o con otro nombre con `Ctrl+Mayús+S`.

En el primer inicio, un breve **recorrido** señala las partes de la ventana. Se puede iniciar de nuevo desde
el botón **Ayuda** (el signo de interrogación arriba a la derecha), que también abre este manual.

## La ventana

- **Barra de título**: el icono y el nombre de la aplicación, el documento abierto y los botones para
  minimizar, maximizar y cerrar la ventana. Arrástrela para mover la ventana; doble clic para maximizarla.
- **Barra superior**: a la izquierda, abrir, guardar, guardar como, deshacer y rehacer; en el centro, el
  nombre del documento, la página y los distintivos (*firmado*, el tipo de protección con contraseña, una
  declaración de PDF/A — haga clic en un distintivo para abrir su diálogo); a la derecha, el tema, las
  **herramientas del documento** (la llave inglesa), la ayuda, los ajustes y el inspector.
- **Barra lateral** (`Ctrl+B`): miniaturas de página, marcadores, fuentes, comentarios, historial.
- **Barra de herramientas** sobre la página: las herramientas descritas abajo, navegación por páginas, zoom,
  búsqueda.
- **Inspector** (`Ctrl+I`): lo que está seleccionado, las opciones de la herramienta activa y las
  propiedades del documento.
- Varios documentos se abren como pestañas. Un PDF abierto desde el Explorador se suma a la ventana que ya
  está abierta.

Cada cambio se puede deshacer (`Ctrl+Z`) hasta que cierre el documento; la pestaña **Historial** enumera los
pasos y permite saltar a cualquiera de ellos. No se escribe nada en el archivo antes de guardar (`Ctrl+S`).

## Leer

Desplácese, o use `Re Pág` / `Av Pág`. `Ctrl`+rueda hace zoom; `Ctrl+0` ajusta al ancho. `Ctrl+F` busca
texto. Con la herramienta **Seleccionar** (`V`) se selecciona y copia texto; la **Mano** (`H`) arrastra la
página.

## Editar texto

Elija **Editar texto e imágenes** (`E`). Cada línea recibe un contorno.

- **Doble clic** en una línea para reescribirla. Si la línea forma parte de un párrafo, se abre el párrafo
  entero y se vuelve a distribuir en su columna al confirmar con `Ctrl+Intro`.
- **Arrastre** una línea para moverla; se ajusta a los bordes y líneas base de otras líneas (`Alt` mueve
  libremente). Las flechas desplazan poco a poco. `Supr` la elimina. `Mayús`+clic o arrastre un recuadro
  para seleccionar varias líneas.
- **Suelte** una línea al final o al principio de otra y ambas **se unen en una sola línea**: la línea
  de destino se resalta mientras arrastra, y cada palabra conserva su fuente, tamaño y color. Mantenga
  `Alt` para dejar una línea junto a otra sin unirlas (*Ajustes › Unir las líneas que se sueltan una junto
  a otra*).
- El inspector cambia el tamaño, el color y la fuente.

Con **Realinear el texto automáticamente** activado en Ajustes (lo está de forma predeterminada), un párrafo
conserva su forma cuando cambia una de sus líneas: acorte una línea y las palabras que continúan su frase
suben a ella; alárguela y lo que sobra pasa a las líneas siguientes; elimine una línea y las de debajo
suben. Solo se mueven las líneas del mismo bloque de texto. Los demás bloques y el resto de la página se
quedan donde están, de modo que un texto que crece una línea puede invadir lo que tiene debajo.

Lo que escribe se compone **en la fuente que el documento ya usa**. Si a esa fuente le falta un carácter,
Inq Studio lo toma del mismo tipo de letra instalado en su equipo; solo si el tipo de letra no aparece por
ningún lado se compone el carácter que falta en una fuente parecida, y un aviso se lo indica. La pestaña
**Fuentes** de la barra lateral muestra, para cada fuente, lo que puede hacer una edición.

**Añadir texto** (`T`): haga clic donde deba empezar, escriba y confirme con `Ctrl+Intro`.

### Texto en escaneos

Una página escaneada es una imagen; sus palabras son píxeles, no texto. Después de *Reconocer texto* (véase
más abajo) las palabras se pueden seleccionar, y también reescribir: doble clic en una línea reconocida con
la herramienta de edición. Como las palabras antiguas forman parte de la imagen, Inq Studio cubre la línea
con un parche del color del papel y dibuja encima su texto en el color de la tinta. El texto nuevo se compone
en Helvetica o Arial, así que no coincidirá exactamente con el tipo de letra del escaneo. El texto reconocido
se edita línea por línea.

## Imágenes

Con la herramienta de edición, haga clic en una imagen para seleccionarla: arrastre para moverla, arrastre
su esquina para cambiar el tamaño, y reemplácela, guárdela o elimínela en el inspector. **Insertar imagen**
(`I`) coloca un PNG o JPEG.

## Comentarios y formas

Resalte (`M`), subraye (`U`) y tache (`K`) arrastrando sobre el texto. La nota (`N`), el dibujo (`D`), el
rectángulo, la elipse, la línea, la flecha, el polígono, el cuadro de texto (`B`), el sello (`S`) y los
enlaces (`L`) funcionan haciendo clic o arrastrando en la página. Con la herramienta Seleccionar, haga clic
en uno para cambiarlo en el inspector o arrástrelo para moverlo.

## Formularios

Rellene los campos de formulario directamente en la página con la herramienta Seleccionar. *Herramientas del
documento › Acoplar campos de formulario* convierte los valores rellenados en contenido normal de la página.

## Páginas

En la pestaña **Páginas**: arrastre las miniaturas para reordenar; clic derecho para girar, duplicar,
insertar una página en blanco, insertar páginas de otro PDF, extraer y eliminar. Las *Herramientas del
documento* añaden marcas de agua, encabezados y pies, numeración Bates, etiquetas de página, el cambio a un
tamaño de papel y la división en varios archivos. **Recortar** (`C`) oculta todo lo que queda fuera de una
zona.

## Censurar

**Censurar** (`X`) es para contenido que debe desaparecer, no solo quedar tapado. Arrastre sobre él — o deje
que *Marcar todo* encuentre cada aparición de una palabra —, revise las marcas y aplíquelas en el inspector.
El texto, los píxeles de las imágenes y los dibujos bajo cada marca se eliminan del propio archivo. Después
de guardar no se pueden recuperar.

## Escaneos: reconocer texto (OCR)

*Herramientas del documento › Reconocer texto*. Elija el idioma del texto y las páginas. Las palabras se
colocan de forma invisible sobre el escaneo, de modo que se puede buscar, seleccionar y copiar; la imagen
queda como está.

El reconocimiento se hace en su equipo con los idiomas de OCR instalados en Windows. Son paquetes de idioma
de Windows y no tienen nada que ver con el idioma de la interfaz de Inq Studio.

- **Detectar automáticamente** (se ofrece cuando hay más de un idioma instalado) lee la primera página con
  texto en cada idioma instalado y se queda con el que encaja.
- **Añadir un idioma a Windows**: elija un idioma y pulse *Instalar…*. Windows descarga el paquete y pide
  derechos de administrador; en un equipo administrado por un centro educativo o una empresa puede que no
  esté permitido. Lo mismo se puede hacer en Windows en *Configuración › Hora e idioma › Idioma y región*.

## Firmas

*Herramientas del documento › Firmas* comprueba cada firma del documento y dice, con palabras claras:

- si el contenido firmado está **sin cambios**, y si se añadió algo al archivo después;
- **quién firmó**, según el certificado, y si su equipo confía en la autoridad que lo emitió.

*Firmar…* firma el documento con una ID digital:

- **De Windows**: un certificado de su almacén de certificados de Windows, incluidas tarjetas inteligentes
  y tokens.
- **De un archivo**: un archivo `.pfx` o `.p12` y su contraseña.
- **Crear una**: una ID autofirmada, guardada como `.pfx`. Demuestra que un documento no ha cambiado desde
  que lo firmó, pero no quién es usted — para eso necesita un certificado de un servicio de confianza.

Una firma puede mostrarse en una página (nombre, fecha, motivo) o ser invisible. Con un **servidor de
sellado de tiempo** indicado en Ajustes, la firma lleva además la hora de una autoridad de sellado de
tiempo, lo que permite seguir comprobándola cuando su certificado caduca.

Al firmar se guarda el documento. Los cambios posteriores se añaden al final del archivo, así que la firma
sigue siendo válida y muestra que se añadió algo después de ella.

## Protección con contraseña

*Herramientas del documento › Proteger con contraseña*. Una **contraseña de apertura** bloquea el documento.
Una **contraseña de permisos** con restricciones (no imprimir, no copiar…) pide a los programas lectores que
se contengan; es una petición, no un candado. Un documento protegido que abre y guarda conserva su
protección.

## Archivos más pequeños, archivo, comparación

- **Optimizar** reduce la resolución de las imágenes que tienen más de la necesaria, combina fuentes e
  imágenes duplicadas y muestra el tamaño antes y después.
- **PDF/A** comprueba el documento con las normas de archivo PDF/A-1b, 2b y 3b, enumera lo que no cumple y
  convierte lo que se puede corregir. Las comprobaciones cubren las reglas que los documentos incumplen en
  la práctica; no son una validación certificada.
- **Comparar con…** muestra dos documentos lado a lado: palabras eliminadas en rojo, palabras añadidas en
  verde, y una lista de los cambios. *Marcar diferencias en la imagen* también encuentra imágenes y gráficos
  modificados.

## Accesibilidad

*Herramientas del documento › Accesibilidad* enumera aquello con lo que tropezarán quienes usan lectores de
pantalla, establece el idioma del documento, permite dar texto alternativo a las figuras y puede dibujar el
orden de lectura sobre la página.

## Exportar e imprimir

*Herramientas del documento › Exportar páginas o texto* guarda páginas como PNG o JPEG con la resolución que
elija, o el texto como archivo de texto. `Ctrl+P` imprime.

## Ajustes

Idioma (inglés, alemán, español, francés; se aplica tras reiniciar), tema, zoom, unidades, su nombre de
autor, cómo se guardan los archivos, la realineación automática del texto, el servidor de sellado de tiempo,
las actualizaciones y dónde está el archivo de registro. Sus documentos permanecen en su equipo: Inq Studio
no sube nada. Las únicas conexiones que hace son la búsqueda de una versión nueva al iniciar y, si indica
uno, el servidor de sellado de tiempo — que recibe un hash, nunca el documento.

Algunos mensajes del motor PDF (ciertos errores y avisos) aún aparecen en inglés.

## Cuando algo va mal

- **Un PDF muestra páginas en blanco.** Actualice a la última versión; antes de la 1.0.1, los archivos de
  algunos escáneres se leían de forma incompleta. Si sigue ocurriendo, puede que el archivo esté dañado:
  otros lectores lo reparan sin avisar.
- **Una línea no se puede editar.** La pestaña *Fuentes* de la barra lateral dice, para cada fuente, lo que
  puede hacer una edición. El texto que forma parte de una imagen necesita antes *Reconocer texto*.
- **Falta el idioma que necesito en el diálogo de OCR.** Instale su paquete con *Añadir un idioma a
  Windows*.
- **El texto escrito es invisible en un escaneo.** Actualice a la 1.0.2 o posterior.
- **La aplicación se cerró de forma inesperada.** El siguiente inicio muestra el informe del fallo. El
  archivo de registro y los informes están solo en su equipo (*Ajustes › Diagnóstico › Mostrar en la
  carpeta*) y nunca contienen el contenido de los documentos. Adjúntelos cuando informe de un problema en
  [github.com/Voltiq/Inq-Studio/issues](https://github.com/Voltiq/Inq-Studio/issues).

## Atajos de teclado

| | |
| --- | --- |
| `Ctrl+O` / `Ctrl+S` / `Ctrl+Mayús+S` | Abrir / guardar / guardar como |
| `Ctrl+Z` / `Ctrl+Y` | Deshacer / rehacer |
| `Ctrl+F` / `Ctrl+P` | Buscar / imprimir |
| `Ctrl+B` / `Ctrl+I` / `Ctrl+,` | Barra lateral / inspector / ajustes |
| `F1` | Este manual |
| `Ctrl+W` / `Ctrl+Tab` | Cerrar pestaña / pestaña siguiente |
| `Ctrl+=` / `Ctrl+-` / `Ctrl+0` | Acercar / alejar / ajustar al ancho |
| `V` `H` `E` `T` `I` | Seleccionar, mano, editar, añadir texto, insertar imagen |
| `M` `U` `K` `N` `D` `B` `S` `L` | Resaltar, subrayar, tachar, nota, dibujar, cuadro de texto, sello, enlaces |
| `X` `C` | Censurar, recortar |

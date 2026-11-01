# kotlinx.serialization and OkHttp ship their own rules.
-dontwarn org.slf4j.**
# named in the phone app's manifest; Cast loads it by name
-keep class io.github.okaza03.valo.cast.ValoCastOptions { *; }

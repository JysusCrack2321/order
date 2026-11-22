# Keep UniFFI / JNA classes intact
-keep class com.sun.jna.** { *; }
-keep class * implements com.sun.jna.** { *; }
-keep class com.jysusman.order.ffi.** { *; }

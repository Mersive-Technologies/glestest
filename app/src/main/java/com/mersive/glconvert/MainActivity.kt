package com.mersive.glconvert

import android.os.Bundle
import android.util.Log
import androidx.appcompat.app.AppCompatActivity
import org.apache.commons.io.IOUtils
import java.io.File
import java.nio.ByteBuffer

class MainActivity : AppCompatActivity() {
    private val tag: String = this.javaClass.name

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.activity_main)

        val texName = "thanksgiving.jpg";
        val bytes = this.classLoader.getResourceAsStream(texName).readBytes()
        File(filesDir.absolutePath, texName).writeBytes(bytes)

        init(filesDir.absolutePath)
    }

    init {
        try {
            System.loadLibrary("glestest")
            Log.i(tag, "Loaded native library!")
        } catch (ex: Exception) {
            Log.e(tag, "Failed to load native library!", ex)
        }
    }

    external fun init(dir: String)

}
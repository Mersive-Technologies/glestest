package com.mersive.glconvert

import android.os.Bundle
import android.util.Log
import androidx.appcompat.app.AppCompatActivity
import java.io.File

class MainActivity : AppCompatActivity() {
    private val tag: String = this.javaClass.name

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.activity_main)

        val texName = "thanksgiving.raw";
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
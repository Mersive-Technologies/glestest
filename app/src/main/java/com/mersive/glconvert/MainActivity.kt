package com.mersive.glconvert

import androidx.appcompat.app.AppCompatActivity
import android.os.Bundle
import android.util.Log

class MainActivity : AppCompatActivity() {
    private val tag: String = this.javaClass.name

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.activity_main)

        init(filesDir.absolutePath);
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